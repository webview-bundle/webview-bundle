use http::Request;
use std::sync::Arc;
use wvb::BundleEntry;
use wvb::protocol::{BundleProtocol, Protocol};
use wvb::remote::ListRemoteBundleInfo;
use wvb::testing::*;
use wvb::updater::Updater;

#[tokio::test]
async fn serve_and_update() {
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
async fn concurrent_requests_same_bundle() {
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
async fn concurrent_requests_multiple_bundles() {
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
async fn unknown_bundle_errors() {
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
async fn unknown_path_returns_404() {
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
async fn non_get_returns_405() {
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
async fn concurrent_unknown_bundle_errors() {
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
async fn repeated_reads_same_content() {
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
async fn concurrent_reads_same_content() {
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
async fn empty_file_served() {
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
async fn many_bundles_concurrent() {
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
async fn special_chars_in_path() {
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
