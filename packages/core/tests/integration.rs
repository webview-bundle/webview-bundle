use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::remote::ListRemoteBundleInfo;
use wvb::source::BundleSource;
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

  updater.download("app", None).await.unwrap();
  source.update_remote_version("app", "1.1.0").await.unwrap();

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

  let mut handles = vec![];

  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    updater.download("app", None).await.unwrap();
    source_clone
      .update_remote_version("app", "2.0.0")
      .await
      .unwrap();
  });

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

  let updater1 = updater.clone();
  let handle1 = tokio::spawn(async move { updater1.download("app1", None).await });

  let updater2 = updater.clone();
  let handle2 = tokio::spawn(async move { updater2.download("app2", None).await });

  handle1.await.unwrap().unwrap();
  handle2.await.unwrap().unwrap();

  source.update_remote_version("app1", "2.0.0").await.unwrap();
  source.update_remote_version("app2", "2.0.0").await.unwrap();

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

  let mut handles = vec![];

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

  let updater = Updater::new(source.clone(), remote.clone(), None);
  let source_clone = source.clone();
  let update_handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    updater.download("app", None).await.unwrap();
    source_clone
      .update_remote_version("app", "2.0.0")
      .await
      .unwrap();
  });

  update_handle.await.unwrap();

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
  }
}

#[tokio::test]
async fn error_handling_bundle_not_found() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let protocol = BundleProtocol::new(source.clone());

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

  assert_eq!(resp.status(), 405);
}

#[tokio::test]
async fn error_handling_concurrent_errors() {
  let system = MockSystem::new();
  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));

  let mut handles = vec![];

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

  let result = updater.get_update("nonexistent").await;
  assert!(result.is_err());
}

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

  let updater = Updater::new(source.clone(), remote.clone(), None);
  updater.download("app", None).await.unwrap();
  source.update_remote_version("app", "2.0.0").await.unwrap();

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

  for i in 1..=5 {
    let version = format!("1.{}.0", i);

    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", &version).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>V{}</h1>", i).as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", &version);

    updater.download("app", None).await.unwrap();
    source.update_remote_version("app", &version).await.unwrap();

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

// =============================================================================
// Safety: Scenario 1 — Protocol serving while a version swap occurs
// =============================================================================
//
// BundleProtocol::handle_inner resolves the bundle filepath once at the start of each
// request and passes that same path to both load_descriptor_at and reader_at.
// This ensures the descriptor and the open file always refer to the same bundle
// version, even if a swap (write_remote_bundle / update_version) happens mid-request.

#[tokio::test]
async fn safety_response_bytes_always_valid_during_concurrent_swap() {
  // V1 and V2 must have different byte lengths so that a descriptor/file mismatch
  // produces a wrong read length -> LZ4 decompression error -> detectable failure.
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] =
    b"<h1>version 2 - significantly longer content to force different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V1, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry("/index.html", BundleEntry::new(V2, "text/html", None)),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let remote = Arc::new(system.remote().get_remote());

  let mut read_handles = vec![];
  for i in 0..200usize {
    let p = protocol.clone();
    let delay_ms = (i % 20) as u64;
    read_handles.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
      p.handle(
        Request::builder()
          .uri("https://app.wvb/index.html")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
    }));
  }

  let updater = Updater::new(source.clone(), remote.clone(), None);
  tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
  updater.download("app", None).await.unwrap();
  // Activation is the actual version swap the concurrent readers race against;
  // a download alone only stages v2 on disk without changing the active version.
  source.update_remote_version("app", "2.0.0").await.unwrap();

  for handle in read_handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V1 || body == V2,
      "response body is neither v1 nor v2 — likely a descriptor/file version mismatch:\n  got: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }
}

#[tokio::test]
async fn safety_manifest_persists_across_source_reload() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>remote</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());

  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();
  // Activate the downloaded version; this persists current_version to the manifest
  // on disk so a fresh source can resolve it after restart.
  source.update_remote_version("app", "2.0.0").await.unwrap();

  // Simulate app restart: build a fresh BundleSource from the same directories.
  let reloaded = Arc::new(
    BundleSource::builder()
      .builtin_dir(builtin_dir)
      .remote_dir(remote_dir)
      .build(),
  );
  let version = reloaded.load_version("app").await.unwrap().unwrap();
  assert_eq!(
    version.version, "2.0.0",
    "downloaded version must survive a source reload"
  );

  let protocol = BundleProtocol::new(reloaded);
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
  assert_eq!(resp.body().as_ref(), b"<h1>remote</h1>");
}

// A download only stages a version on disk; it never changes which version the protocol
// serves. `insert_entry` leaves current_version untouched (None for a brand-new entry),
// so the source keeps resolving to its previous/builtin version until the caller invokes
// `update_version`. This holds for both the first download (or_insert_with) and later
// downloads of an already-present bundle (and_modify).
#[tokio::test]
async fn safety_download_stages_without_activating_until_update_version() {
  const V1_CONTENT: &[u8] = b"<h1>v1.1.0</h1>";
  const V2_CONTENT: &[u8] = b"<h1>v2.0.0 - longer content to ensure different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "1.1.0").with_entry(
      "/index.html",
      BundleEntry::new(V1_CONTENT, "text/html", None),
    ))
    .set_bundle_current_version("app", "1.1.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());

  // First download stages 1.1.0 (current_version stays None); activate it explicitly
  // to establish the baseline the protocol serves.
  Updater::new(source.clone(), remote.clone(), None)
    .download("app", None)
    .await
    .unwrap();
  source.update_remote_version("app", "1.1.0").await.unwrap();

  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(V2_CONTENT, "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let protocol = BundleProtocol::new(source.clone());

  // Second download stages 2.0.0 on disk (and_modify branch) but must not switch the
  // active version: the protocol must keep serving the still-active 1.1.0.
  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();

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
    resp.body().as_ref(),
    V1_CONTENT,
    "a download alone must not change the active version"
  );

  // Explicit activation switches the protocol to the staged bundle.
  source.update_remote_version("app", "2.0.0").await.unwrap();

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
    resp.body().as_ref(),
    V2_CONTENT,
    "update_version activates the staged bundle"
  );
}

// V1 and V2 must have different content lengths. If they were the same length, the LZ4
// compressed sizes would be equal, the stale v1 descriptor would read the correct byte
// count from the v2 file, and the bug would be invisible to this test.
#[tokio::test]
async fn safety_descriptor_cache_invalidated_after_activation() {
  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] = b"<h1>version 2 - longer content ensures different LZ4 compressed size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V1, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry("/index.html", BundleEntry::new(V2, "text/html", None)),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());

  // Warm up the descriptor cache with v1.
  let warm = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(warm.body().as_ref(), V1);

  // Download stages v2; activation makes it the served version.
  Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await
    .unwrap();
  // The active version (and thus the resolved filepath) changes here. Without cache
  // invalidation the stale v1 descriptor (entry.len()=L1) would read the v2 file with
  // the wrong byte count (L1 != L2) -> LZ4 decompression fails -> .unwrap() panics.
  source.update_remote_version("app", "2.0.0").await.unwrap();

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
    resp.body().as_ref(),
    V2,
    "descriptor cache was not invalidated after activation"
  );
}

// Proves that a descriptor/file version mismatch produces a hard error rather than
// silently returning wrong bytes. This verifies that the concurrent swap test's
// failure would be detectable (an error, not just wrong bytes).
#[tokio::test]
async fn safety_descriptor_file_mismatch_produces_hard_error() {
  use tokio::fs::File;
  use wvb::source::BundleSource;

  const V1: &[u8] = b"<h1>v1</h1>";
  const V2: &[u8] = b"<h1>version 2 - longer content to guarantee different LZ4 size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V1, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  // Write v2 bundle file directly, bypassing write_remote_bundle so the manifest still
  // points to v1 — simulates the v2 file being present but not yet active.
  let v2_bundle = MockBundle::new("app", "2.0.0")
    .with_entry("/index.html", BundleEntry::new(V2, "text/html", None));
  let v2_dir = remote_dir.join("app");
  std::fs::create_dir_all(&v2_dir).unwrap();
  let v2_path = v2_dir.join("app_2.0.0.wvb");
  std::fs::write(&v2_path, v2_bundle.bundle_data()).unwrap();

  let source_v1 = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(&remote_dir)
    .build();

  // Load the v1 descriptor — what a task holds after a cache hit.
  let v1_descriptor = source_v1.fetch_descriptor("app").await.unwrap();

  // Open the v2 file — what source.reader() returns after version is bumped in-memory
  // but before unload_descriptor clears the cache.
  let v2_reader = File::open(&v2_path).await.unwrap();

  // v1 entry.len() = compressed_size(V1) != compressed_size(V2)
  // -> reads wrong number of bytes -> LZ4 decompression error
  let result = v1_descriptor.async_get_data(v2_reader, "/index.html").await;
  assert!(
    result.is_err(),
    "using a v1 descriptor to read a v2 file must produce an error, not silently return wrong data"
  );
}

// =============================================================================
// Safety: Scenario 2 — FS error fail-over
// =============================================================================

// When the .wvb file listed in the manifest has been deleted from disk,
// the protocol must return BundleNotFound rather than panic or silent garbage.
#[tokio::test]
async fn safety_missing_wvb_file_returns_bundle_not_found() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(b"hello", "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  // Delete the .wvb file but leave the manifest intact.
  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::remove_file(&wvb_path).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let err = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await
    .unwrap_err();
  assert!(
    matches!(err, wvb::Error::BundleNotFound),
    "expected BundleNotFound, got: {err}"
  );
}

// When manifest.json contains invalid JSON (e.g. truncated on a crash),
// the source must behave as if the directory is empty — no panic, no corrupt state.
#[tokio::test]
async fn safety_corrupted_manifest_treated_as_empty() {
  let temp = TempDir::new();
  let builtin_dir = temp.dir().join("builtin");
  let remote_dir = temp.dir().join("remote");
  std::fs::create_dir_all(&builtin_dir).unwrap();
  std::fs::create_dir_all(&remote_dir).unwrap();

  std::fs::write(
    builtin_dir.join("manifest.json"),
    b"{ this is not valid json ",
  )
  .unwrap();

  let source = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(&remote_dir)
    .build();

  let result = source.load_version("app").await;
  assert!(
    result.is_err(),
    "corrupted manifest must return an error, not silently produce None"
  );
}

// When a .wvb file exists on disk but its bytes are random garbage (e.g. partial write
// from a power loss), reads must fail with a parse error — not a panic or silent wrong data.
#[tokio::test]
async fn safety_corrupted_bundle_file_returns_error() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>hello</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::write(&wvb_path, b"this is not a valid wvb file at all !!!").unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(
    result.is_err(),
    "corrupted bundle file must return an error, not a 200 with garbage"
  );
}

// A zero-byte file must also be rejected gracefully (no panic, no 200).
#[tokio::test]
async fn safety_empty_bundle_file_returns_error() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(b"hello", "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();
  std::fs::write(builtin_dir.join("app").join("app_1.0.0.wvb"), b"").unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(result.is_err(), "empty .wvb file must return an error");
}

// A truncated file (simulates interrupted download / power loss mid-write)
// must be rejected without panicking.
#[tokio::test]
async fn safety_truncated_bundle_file_returns_error() {
  let mut system = MockSystem::new();
  let bundle = MockBundle::new("app", "1.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"<h1>content</h1>", "text/html", None),
  );
  system
    .source_mut()
    .add_builtin_bundle(bundle.clone())
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let valid_bytes = bundle.bundle_data();
  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  std::fs::write(&wvb_path, &valid_bytes[..10.min(valid_bytes.len())]).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(result.is_err(), "truncated .wvb file must return an error");
}

// A .wvb file present on disk without a manifest entry must not be visible to load_version.
// This matches the crash-before-save scenario: the file is written but the manifest was
// never flushed, so on restart the remote dir looks empty.
#[tokio::test]
async fn safety_bundle_without_manifest_entry_is_not_visible() {
  let temp = TempDir::new();
  let builtin_dir = temp.dir().join("builtin");
  let remote_dir = temp.dir().join("remote").join("app");
  std::fs::create_dir_all(&builtin_dir).unwrap();
  std::fs::create_dir_all(&remote_dir).unwrap();

  // Drop a .wvb file directly (bypassing write_remote_bundle, so no manifest entry).
  let bundle = MockBundle::new("app", "2.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"orphan", "text/html", None),
  );
  std::fs::write(remote_dir.join("app_2.0.0.wvb"), bundle.bundle_data()).unwrap();

  let source = BundleSource::builder()
    .builtin_dir(&builtin_dir)
    .remote_dir(temp.dir().join("remote"))
    .build();

  let version = source.load_version("app").await.unwrap();
  assert!(
    version.is_none(),
    "a .wvb file without a manifest entry must not be visible to load_version"
  );
}

// =============================================================================
// Safety: Scenario 3 — Downloaded bundle integrity fail-over
// =============================================================================
//
// The `integrity` feature (SHA-3 hash check) is not enabled in the default tauri plugin
// build. The tests below verify what the system guarantees WITHOUT that feature:
// structural validity (magic bytes, checksum, framing) is always checked via the binary
// format parser; semantic integrity (hash matches advertised value) is opt-in.

// The binary format has a fixed magic number and internal checksums. A bundle whose bytes
// have been modified after creation must fail during parse.
#[tokio::test]
async fn safety_bit_flipped_bundle_file_fails_parse() {
  let mut system = MockSystem::new();
  let bundle = MockBundle::new("app", "1.0.0").with_entry(
    "/index.html",
    BundleEntry::new(b"<h1>original</h1>", "text/html", None),
  );
  system
    .source_mut()
    .add_builtin_bundle(bundle.clone())
    .set_builtin_current_version("app", "1.0.0");

  let (builtin_dir, remote_dir) = system.source().dirs();

  let wvb_path = builtin_dir.join("app").join("app_1.0.0.wvb");
  let mut bytes = std::fs::read(&wvb_path).unwrap();
  let mid = bytes.len() / 2;
  bytes[mid] ^= 0xFF;
  std::fs::write(&wvb_path, &bytes).unwrap();

  let source = Arc::new(
    BundleSource::builder()
      .builtin_dir(&builtin_dir)
      .remote_dir(&remote_dir)
      .build(),
  );
  let protocol = BundleProtocol::new(source.clone());

  let result = protocol
    .handle(
      Request::builder()
        .uri("https://app.wvb/index.html")
        .method("GET")
        .body(vec![])
        .unwrap(),
    )
    .await;
  assert!(
    result.is_err(),
    "a bit-flipped bundle must be rejected, not silently served"
  );
}

// Verifies that the download path does not silently swallow network errors:
// if the server returns 404, download_update must propagate the error.
#[tokio::test]
async fn safety_remote_bundle_not_found_propagates_error() {
  let system = MockSystem::new(); // remote is empty
  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(source, remote, None);

  let err = updater.download("nonexistent", None).await.unwrap_err();
  assert!(
    matches!(err, wvb::Error::RemoteBundleNotFound),
    "expected RemoteBundleNotFound, got: {err}"
  );
}

// After a failed download, the source must remain in its previous state.
#[tokio::test]
async fn safety_failed_download_does_not_corrupt_existing_source() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>stable</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  // remote has no "app" bundle -> download will fail with RemoteBundleNotFound

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());

  let _ = Updater::new(source.clone(), remote, None)
    .download("app", None)
    .await; // intentionally ignore error

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
  assert_eq!(resp.body().as_ref(), b"<h1>stable</h1>");
}

// =============================================================================
// Update flow: download (stage) + install (activate + prune)
// =============================================================================

fn get(uri: &str) -> Request<Vec<u8>> {
  Request::builder()
    .uri(uri)
    .method("GET")
    .body(vec![])
    .unwrap()
}

// download stages on disk without serving it; install activates it.
#[tokio::test]
async fn install_activates_staged_version() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote, None);

  // Download stages 2.0.0 but the protocol keeps serving the builtin.
  updater.download("app", None).await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>builtin</h1>");

  // Install activates it.
  updater.install("app", "2.0.0").await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>v2</h1>");
}

// Installing a version that was never downloaded (no manifest entry) must error.
#[tokio::test]
async fn install_unknown_version_errors() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let updater = Updater::new(source.clone(), remote, None);

  let err = updater.install("app", "9.9.9").await.unwrap_err();
  assert!(
    matches!(err, wvb::Error::BundleEntryNotExists { .. }),
    "expected BundleEntryNotExists, got: {err}"
  );
}

// Each install keeps {current, previous} and prunes older staged versions; a previous
// version stays on disk so a one-step rollback can re-activate it.
#[tokio::test]
async fn install_prunes_old_and_supports_rollback() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote, None);

  // Standard flow: download then install, one version at a time.
  for v in ["1.1.0", "1.2.0", "1.3.0"] {
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", v).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>{v}</h1>").as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", v);
    updater.download("app", None).await.unwrap();
    updater.install("app", v).await.unwrap();
    let resp = protocol
      .handle(get("https://app.wvb/index.html"))
      .await
      .unwrap();
    assert_eq!(resp.body().as_ref(), format!("<h1>{v}</h1>").as_bytes());
  }

  // After installing 1.3.0: keep {1.3.0 (current), 1.2.0 (previous)}, prune 1.1.0.
  let mut retained = source.remote_retained_versions("app").await.unwrap();
  retained.sort();
  assert_eq!(retained, vec!["1.2.0".to_string(), "1.3.0".to_string()]);
  assert!(
    source
      .load_remote_metadata("app", "1.1.0")
      .await
      .unwrap()
      .is_none()
  );

  let (_, remote_dir) = system.source().dirs();
  assert!(!remote_dir.join("app").join("app_1.1.0.wvb").exists());
  assert!(remote_dir.join("app").join("app_1.2.0.wvb").exists());
  assert!(remote_dir.join("app").join("app_1.3.0.wvb").exists());

  // Roll back to the retained previous version: file is still present, so it succeeds.
  updater.install("app", "1.2.0").await.unwrap();
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>1.2.0</h1>");
}

// A staged bundle whose file is corrupt on disk must fail install (structural parse),
// leaving the previously active version untouched.
#[tokio::test]
async fn install_rejects_corrupt_on_disk_bundle() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(MockBundle::new("app", "2.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>v2</h1>", "text/html", None),
    ))
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Updater::new(source.clone(), remote, None);

  updater.download("app", None).await.unwrap();

  // Corrupt the staged file on disk.
  let (_, remote_dir) = system.source().dirs();
  std::fs::write(
    remote_dir.join("app").join("app_2.0.0.wvb"),
    b"not a valid wvb file",
  )
  .unwrap();

  let err = updater.install("app", "2.0.0").await.unwrap_err();
  assert!(!matches!(err, wvb::Error::BundleEntryNotExists { .. }));

  // Activation never happened: the builtin is still served.
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>builtin</h1>");
}

// While install activates a new version, concurrent protocol requests must always get a
// present, valid bundle — never a BundleNotFound. The just-replaced version is retained
// and the descriptor pins its own filepath, so in-flight reads keep resolving.
#[tokio::test]
async fn install_during_concurrent_reads_never_serves_missing() {
  const V0: &[u8] = b"<h1>builtin</h1>";
  const V2: &[u8] = b"<h1>v2 - longer body to force a different compressed size</h1>";

  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(
      MockBundle::new("app", "1.0.0")
        .with_entry("/index.html", BundleEntry::new(V0, "text/html", None)),
    )
    .set_builtin_current_version("app", "1.0.0");
  system
    .remote_mut()
    .add_bundle(
      MockBundle::new("app", "2.0.0")
        .with_entry("/index.html", BundleEntry::new(V2, "text/html", None)),
    )
    .set_bundle_current_version("app", "2.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = Arc::new(BundleProtocol::new(source.clone()));
  let updater = Updater::new(source.clone(), remote, None);

  // Stage v2 on disk (not yet active).
  updater.download("app", None).await.unwrap();

  let mut reads = vec![];
  for i in 0..200usize {
    let p = protocol.clone();
    let delay = (i % 20) as u64;
    reads.push(tokio::spawn(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
      p.handle(get("https://app.wvb/index.html")).await
    }));
  }

  tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
  updater.install("app", "2.0.0").await.unwrap();

  for r in reads {
    // .unwrap().unwrap(): the request must neither panic nor return an error.
    let resp = r.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.body().as_ref();
    assert!(
      body == V0 || body == V2,
      "served neither the old nor the new bundle: {:?}",
      std::str::from_utf8(body).unwrap_or("<binary>")
    );
  }
}

// Two installs racing on the same bundle must serialize (per-bundle lock) and leave a
// consistent state: the protocol serves a present, valid version (no torn current
// pointing at a pruned file). The loser may fail because the winner pruned its target.
#[tokio::test]
async fn concurrent_installs_serialize_to_consistent_state() {
  let mut system = MockSystem::new();
  system
    .source_mut()
    .add_builtin_bundle(MockBundle::new("app", "1.0.0").with_entry(
      "/index.html",
      BundleEntry::new(b"<h1>builtin</h1>", "text/html", None),
    ))
    .set_builtin_current_version("app", "1.0.0");

  let source = Arc::new(system.source().get_source());
  let remote = Arc::new(system.remote().get_remote());
  let protocol = BundleProtocol::new(source.clone());
  let updater = Arc::new(Updater::new(source.clone(), remote, None));

  for v in ["1.1.0", "1.2.0"] {
    system
      .remote_mut()
      .add_bundle(MockBundle::new("app", v).with_entry(
        "/index.html",
        BundleEntry::new(format!("<h1>{v}</h1>").as_bytes(), "text/html", None),
      ))
      .set_bundle_current_version("app", v);
    updater.download("app", None).await.unwrap();
  }

  let u1 = updater.clone();
  let h1 = tokio::spawn(async move { u1.install("app", "1.1.0").await });
  let u2 = updater.clone();
  let h2 = tokio::spawn(async move { u2.install("app", "1.2.0").await });
  let r1 = h1.await.unwrap();
  let r2 = h2.await.unwrap();

  // At least one install wins; the loser may error (its target was pruned by the winner).
  assert!(r1.is_ok() || r2.is_ok(), "both installs failed");

  // Whatever the interleaving, the active version resolves to a present, valid bundle.
  let resp = protocol
    .handle(get("https://app.wvb/index.html"))
    .await
    .unwrap();
  assert_eq!(resp.status(), 200);
  let body = resp.body().as_ref();
  assert!(
    body == b"<h1>1.1.0</h1>" || body == b"<h1>1.2.0</h1>",
    "served neither installed version"
  );
}
