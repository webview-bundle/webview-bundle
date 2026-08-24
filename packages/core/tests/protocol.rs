mod common;

use common::{INDEX, builtin_source, bundle, bundle_of, get, remote_server, update_all, updater};
use std::sync::Arc;
use wvb::protocol::{BundleProtocol, Protocol};

#[tokio::test]
async fn serve_and_update() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>1.0.0</h1>")]);
  let server = remote_server(vec![bundle("app", "1.1.0", b"<h1>1.1.0</h1>")]);
  let protocol = BundleProtocol::new(source.clone());

  let resp = protocol.handle(get("https://app.wvb")).await.unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>1.0.0</h1>");

  let updater = updater(&source, &server.base_url());
  let updated = update_all(&updater).await;
  assert_eq!(updated, vec![("app".to_owned(), "1.1.0".to_owned())]);

  let resp = protocol.handle(get("https://app.wvb")).await.unwrap();
  assert_eq!(resp.body().as_ref(), b"<h1>1.1.0</h1>");
}

#[tokio::test]
async fn concurrent_requests_same_bundle() {
  let source = builtin_source(vec![bundle_of(
    "app",
    "1.0.0",
    &[
      (INDEX, b"<h1>Index</h1>"),
      ("/page1.html", b"<h1>Page 1</h1>"),
      ("/page2.html", b"<h1>Page 2</h1>"),
      ("/page3.html", b"<h1>Page 3</h1>"),
    ],
  )]);
  let protocol = Arc::new(BundleProtocol::new(source));

  let mut handles = vec![];
  for i in 0..100 {
    let protocol = protocol.clone();
    let path = match i % 4 {
      0 => INDEX,
      1 => "/page1.html",
      2 => "/page2.html",
      _ => "/page3.html",
    };
    handles.push(tokio::spawn(async move {
      protocol
        .handle(get(&format!("https://app.wvb{path}")))
        .await
    }));
  }

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    assert!(str::from_utf8(resp.body()).unwrap().starts_with("<h1>"));
  }
}

#[tokio::test]
async fn concurrent_requests_multiple_bundles() {
  let source = builtin_source(vec![
    bundle("app1", "1.0.0", b"<h1>App 1</h1>"),
    bundle("app2", "1.0.0", b"<h1>App 2</h1>"),
    bundle("app3", "1.0.0", b"<h1>App 3</h1>"),
  ]);
  let protocol = Arc::new(BundleProtocol::new(source));

  let mut handles = vec![];
  for i in 0..90 {
    let protocol = protocol.clone();
    let bundle_name = match i % 3 {
      0 => "app1",
      1 => "app2",
      _ => "app3",
    };
    handles.push(tokio::spawn(async move {
      protocol
        .handle(get(&format!("https://{bundle_name}.wvb/index.html")))
        .await
    }));
  }

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    assert!(str::from_utf8(resp.body()).unwrap().starts_with("<h1>App "));
  }
}

#[tokio::test]
async fn unknown_bundle_errors() {
  let protocol = BundleProtocol::new(builtin_source(vec![]));

  let err = protocol
    .handle(get("https://nonexistent.wvb/index.html"))
    .await
    .unwrap_err();

  assert!(matches!(err, wvb::Error::BundleNotFound));
}

#[tokio::test]
async fn unknown_path_returns_404() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>Index</h1>")]);
  let protocol = BundleProtocol::new(source);

  let resp = protocol
    .handle(get("https://app.wvb/nonexistent.html"))
    .await
    .unwrap();

  assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn non_get_returns_405() {
  let source = builtin_source(vec![bundle("app", "1.0.0", b"<h1>Index</h1>")]);
  let protocol = BundleProtocol::new(source);

  let resp = protocol
    .handle(
      http::Request::builder()
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
  let protocol = Arc::new(BundleProtocol::new(builtin_source(vec![])));

  let mut handles = vec![];
  for _ in 0..50 {
    let protocol = protocol.clone();
    handles.push(tokio::spawn(async move {
      protocol
        .handle(get("https://nonexistent.wvb/index.html"))
        .await
    }));
  }

  for handle in handles {
    let err = handle.await.unwrap().unwrap_err();
    assert!(matches!(err, wvb::Error::BundleNotFound));
  }
}

#[tokio::test]
async fn repeated_reads_same_content() {
  const BODY: &[u8] = b"<h1>Test Content 12345</h1>";

  let source = builtin_source(vec![bundle_of("app", "1.0.0", &[("/test.html", BODY)])]);
  let protocol = BundleProtocol::new(source);

  for _ in 0..100 {
    let resp = protocol
      .handle(get("https://app.wvb/test.html"))
      .await
      .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), BODY);
  }
}

#[tokio::test]
async fn concurrent_reads_same_content() {
  const BODY: &[u8] = b"<h1>Concurrent Test</h1>";

  let source = builtin_source(vec![bundle_of("app", "1.0.0", &[("/test.html", BODY)])]);
  let protocol = Arc::new(BundleProtocol::new(source));

  let mut handles = vec![];
  for _ in 0..100 {
    let protocol = protocol.clone();
    handles.push(tokio::spawn(async move {
      protocol.handle(get("https://app.wvb/test.html")).await
    }));
  }

  for handle in handles {
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), BODY);
  }
}

#[tokio::test]
async fn empty_file_served() {
  let source = builtin_source(vec![bundle_of("app", "1.0.0", &[("/empty.txt", b"")])]);
  let protocol = BundleProtocol::new(source);

  let resp = protocol
    .handle(get("https://app.wvb/empty.txt"))
    .await
    .unwrap();

  assert_eq!(resp.status(), 200);
  assert_eq!(resp.body().len(), 0);
}

#[tokio::test]
async fn many_bundles_concurrent() {
  let bundles = (1..=10)
    .map(|i| {
      bundle(
        &format!("app{i}"),
        "1.0.0",
        format!("<h1>App {i}</h1>").as_bytes(),
      )
    })
    .collect();
  let protocol = Arc::new(BundleProtocol::new(builtin_source(bundles)));

  let mut handles = vec![];
  for i in 0..200 {
    let protocol = protocol.clone();
    let bundle_name = format!("app{}", (i % 10) + 1);
    handles.push(tokio::spawn(async move {
      protocol
        .handle(get(&format!("https://{bundle_name}.wvb/index.html")))
        .await
    }));
  }

  for handle in handles {
    assert_eq!(handle.await.unwrap().unwrap().status(), 200);
  }
}

#[tokio::test]
async fn special_chars_in_path() {
  let source = builtin_source(vec![bundle_of(
    "app",
    "1.0.0",
    &[
      ("/path with spaces.html", b"<h1>Spaces</h1>"),
      ("/path-with-dashes.html", b"<h1>Dashes</h1>"),
      ("/path_with_underscores.html", b"<h1>Underscores</h1>"),
    ],
  )]);
  let protocol = BundleProtocol::new(source);

  for uri in [
    "https://app.wvb/path%20with%20spaces.html",
    "https://app.wvb/path-with-dashes.html",
    "https://app.wvb/path_with_underscores.html",
  ] {
    let resp = protocol.handle(get(uri)).await.unwrap();
    assert_eq!(resp.status(), 200, "{uri}");
  }
}
