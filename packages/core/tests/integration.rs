use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::remote::ListRemoteBundleInfo;
use wvb::testing::*;
use wvb::updater::Updater;

#[tokio::test]
async fn protocol_simple() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>1.0.0</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "1.1.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>1.1.0</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "1.1.0");

  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  let html = str::from_utf8(resp.body()).unwrap();
  assert_eq!(html, "<h1>1.0.0</h1>");

  let remote = Arc::new(system.remote().get_remote());

  let updater = Updater::new(source.clone(), remote.clone(), None);
  let remotes = updater.list_remotes().await.unwrap();
  assert_eq!(remotes.len(), 1);
  assert_eq!(
    remotes,
    vec![ListRemoteBundleInfo {
      name: "app".to_string(),
      version: "1.1.0".to_string(),
    }]
  );
  let update_info = updater.get_update("app").await.unwrap();
  assert_eq!(update_info.name, "app");
  assert_eq!(update_info.version, "1.1.0");
  assert_eq!(update_info.local_version.unwrap(), "1.0.0");
  assert!(update_info.is_available);

  updater.download_update("app", None).await.unwrap();
  source.update_version("app", "1.1.0").await.unwrap();

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  let html = str::from_utf8(resp.body()).unwrap();
  assert_eq!(html, "<h1>1.1.0</h1>");
}

#[tokio::test]
async fn protocol_handle_concurrent_requests() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry(
          "/index.html",
          BundleEntry::new(b"<h1>Index</h1>", "text/html", None),
        )
        .with_entry(
          "/page1.html",
          BundleEntry::new(b"<h1>Page 1</h1>", "text/html", None),
        )
        .with_entry(
          "/page2.html",
          BundleEntry::new(b"<h1>Page 2</h1>", "text/html", None),
        )
        .with_entry(
          "/page3.html",
          BundleEntry::new(b"<h1>Page 3</h1>", "text/html", None),
        ),
    )
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  // Spawn 100 concurrent requests
  let mut handles = vec![];
  for i in 0..100 {
    let protocol = protocol.clone();
    let path = match i % 4 {
      0 => "/index.html",
      1 => "/page1.html",
      2 => "/page2.html",
      _ => "/page3.html",
    };
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri(format!("https://app.wvb{}", path))
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Verify all requests succeeded
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let html = str::from_utf8(resp.body()).unwrap();
    assert!(html.starts_with("<h1>"));
  }
}

#[tokio::test]
async fn protocol_handle_during_bundle_update() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 1.0.0</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 2.0.0</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  // Request before update
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(
    str::from_utf8(resp.body()).unwrap(),
    "<h1>Version 1.0.0</h1>"
  );

  // Spawn multiple concurrent requests during update
  let mut handles = vec![];

  // Run the update task asynchronously
  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    updater.download_update("app", None).await.unwrap();
    source_clone.update_version("app", "2.0.0").await.unwrap();
  });

  // Spawn concurrent requests during update
  for _ in 0..50 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  update_handle.await.unwrap();

  // Verify all requests succeeded (either 1.0.0 or 2.0.0)
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let html = str::from_utf8(resp.body()).unwrap();
    assert!(
      html == "<h1>Version 1.0.0</h1>" || html == "<h1>Version 2.0.0</h1>",
      "Unexpected response: {}",
      html
    );
  }

  // Requests after update should return 2.0.0
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(
    str::from_utf8(resp.body()).unwrap(),
    "<h1>Version 2.0.0</h1>"
  );
}

#[tokio::test]
async fn protocol_handle_multiple_bundles_concurrent() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app1", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app1", "1.0.0")
    .add_builtin_bundle(MockBundle::new("app2", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 2</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app2", "1.0.0")
    .add_builtin_bundle(MockBundle::new("app3", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 3</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app3", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  // Concurrent requests across multiple bundles
  let mut handles = vec![];
  for i in 0..90 {
    let protocol = protocol.clone();
    let app_name = match i % 3 {
      0 => "app1",
      1 => "app2",
      _ => "app3",
    };
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri(format!("https://{}.wvb/index.html", app_name))
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Verify all requests
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let html = str::from_utf8(resp.body()).unwrap();
    assert!(html.starts_with("<h1>App "));
  }
}

#[tokio::test]
async fn updater_concurrent_updates() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app1", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 1 v1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app1", "1.0.0")
    .add_builtin_bundle(MockBundle::new("app2", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 2 v1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app2", "1.0.0");

  system
    .remote_mut()
    .add_bundle(MockBundle::new("app1", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 1 v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app1", "2.0.0")
    .add_bundle(MockBundle::new("app2", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>App 2 v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app2", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Arc::new(Updater::new(source.clone(), remote.clone(), None));

  // Update multiple bundles concurrently
  let updater1 = updater.clone();
  let handle1 = tokio::spawn(async move { updater1.download_update("app1", None).await });

  let updater2 = updater.clone();
  let handle2 = tokio::spawn(async move { updater2.download_update("app2", None).await });

  // Verify all updates succeeded
  handle1.await.unwrap().unwrap();
  handle2.await.unwrap().unwrap();

  // Apply version updates
  source.update_version("app1", "2.0.0").await.unwrap();
  source.update_version("app2", "2.0.0").await.unwrap();

  // Verify via protocol
  let protocol = BundleProtocol::new(source.clone());
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app1.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>App 1 v2</h1>");

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app2.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>App 2 v2</h1>");
}

#[tokio::test]
async fn protocol_and_updater_stress_test() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  // Continuously send protocol requests while performing concurrent updates
  let mut handles = vec![];

  // 100 protocol requests
  for _ in 0..100 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Perform update concurrently
  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    updater.download_update("app", None).await.unwrap();
    source_clone.update_version("app", "2.0.0").await.unwrap();
  });

  update_handle.await.unwrap();

  // Verify all requests succeeded
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
  }
}

// === Error Handling and Safety Tests ===

#[tokio::test]
async fn error_handling_bundle_not_found() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

  // Request a non-existent bundle
  let result = protocol
    .handle(
      Request::builder()
        .uri("https://nonexistent.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;

  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), wvb::Error::BundleNotFound));
}

#[tokio::test]
async fn error_handling_file_not_found() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Index</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

  // Request a non-existent file
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/nonexistent.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn error_handling_invalid_method() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Index</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

  // POST method is not supported
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("POST")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(resp.status(), 405); // Method Not Allowed
}

#[tokio::test]
async fn error_handling_concurrent_errors() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  let mut handles = vec![];

  // Spawn 50 concurrent error requests
  for _ in 0..50 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri("https://nonexistent.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Verify all requests return an error
  for handle in handles {
    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), wvb::Error::BundleNotFound));
  }
}

#[tokio::test]
async fn updater_error_handling_remote_not_found() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(source.clone(), remote.clone(), None);

  // Attempt to update a non-existent bundle
  let result = updater.get_update("nonexistent").await;
  assert!(result.is_err());
}

// === Data Integrity Tests ===

#[tokio::test]
async fn data_integrity_content_verification() {
  let mut system = MockSystem::new();
  let expected_content = b"<h1>Test Content 12345</h1>";
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/test.html",
      BundleEntry::new(expected_content, "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  // Verify content matches over 100 iterations
  for _ in 0..100 {
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/test.html")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), expected_content);
  }
}

#[tokio::test]
async fn data_integrity_concurrent_reads() {
  let mut system = MockSystem::new();
  let expected_content = b"<h1>Concurrent Test</h1>";
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/test.html",
      BundleEntry::new(expected_content, "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  let mut handles = vec![];

  // 100 concurrent read requests
  for _ in 0..100 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/test.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Verify all requests return identical content
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), expected_content);
  }
}

#[tokio::test]
async fn data_integrity_update_atomicity() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>Version 2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  // Verify version before update
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>Version 1</h1>");

  // Perform update
  let updater = Updater::new(source.clone(), remote.clone(), None);
  updater.download_update("app", None).await.unwrap();
  source.update_version("app", "2.0.0").await.unwrap();

  // Verify all requests return the new version after update
  let mut handles = vec![];
  for _ in 0..50 {
    let protocol = protocol.clone();
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(str::from_utf8(resp.body()).unwrap(), "<h1>Version 2</h1>");
  }
}

// === Resource Management Tests ===

#[tokio::test]
async fn resource_cleanup_multiple_updates() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>V1</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote.clone(), None);

  // Update sequentially
  for i in 1..=5 {
    let version = format!("1.{}.0", i);

    // Add new version to remote and set as current on each iteration
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", &version).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>V{}</h1>", i).as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", &version);

    updater.download_update("app", None).await.unwrap();
    source.update_version("app", &version).await.unwrap();

    // Verify normal operation after each update
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/index.html")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 200);
    let content = str::from_utf8(resp.body()).unwrap();
    assert_eq!(content, format!("<h1>V{}</h1>", i));
  }
}

// === Boundary Condition Tests ===

#[tokio::test]
async fn boundary_empty_file() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/empty.txt", BundleEntry::new(b"", "text/plain", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/empty.txt")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().len(), 0);
}

#[tokio::test]
async fn boundary_large_concurrent_load() {
  let mut system = MockSystem::new();

  // Create 10 bundles
  for i in 1..=10 {
    let bundle_name = format!("app{}", i);
    system
      .source_mut()
      .add_builtin_bundle(MockBundle::new(&bundle_name, "1.0.0").with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>App {}</h1>", i).as_bytes(), "text/html", None),
      ))
      .set_builtin_current_version(&bundle_name, "1.0.0");
  }

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  let mut handles = vec![];

  // Distribute 200 requests across 10 bundles
  for i in 0..200 {
    let protocol = protocol.clone();
    let bundle_name = format!("app{}", (i % 10) + 1);
    let handle = tokio::spawn(async move {
      protocol
        .handle(
          Request::builder()
            .uri(format!("https://{}.wvb/index.html", bundle_name))
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
    });
    handles.push(handle);
  }

  // Verify all requests succeed
  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
  }
}

#[tokio::test]
async fn boundary_special_characters_in_path() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry(
          "/path with spaces.html",
          BundleEntry::new(b"<h1>Spaces</h1>", "text/html", None),
        )
        .with_entry(
          "/path-with-dashes.html",
          BundleEntry::new(b"<h1>Dashes</h1>", "text/html", None),
        )
        .with_entry(
          "/path_with_underscores.html",
          BundleEntry::new(b"<h1>Underscores</h1>", "text/html", None),
        ),
    )
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

  // Test URL-encoded path
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/path%20with%20spaces.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);

  // Path with dashes
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/path-with-dashes.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);

  // Path with underscores
  let resp = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/path_with_underscores.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);
}
