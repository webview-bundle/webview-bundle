use std::time::{Duration, Instant};
use wvb::remote::{HttpOptions, Remote};
use wvb::testing::TempDir;

#[tokio::test]
async fn download_times_out() {
  // Server that reads the request then sleeps forever without responding.
  let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
  let port = server.server_addr().to_ip().unwrap().port();
  std::thread::spawn(move || {
    for request in server.incoming_requests() {
      std::thread::sleep(Duration::from_secs(30));
      drop(request);
    }
  });

  let base_url = format!("http://127.0.0.1:{port}");
  let remote = Remote::builder()
    .base_url(&base_url)
    .http(HttpOptions::new().timeout(150))
    .build()
    .unwrap();
  let temp = TempDir::new();

  let start = Instant::now();
  let result = remote
    .download(
      format!("{base_url}/bundles/app/1.0.0"),
      &temp.dir().join("app.wvb"),
      None,
    )
    .await;
  let elapsed = start.elapsed();

  assert!(
    result.is_err(),
    "download against an unresponsive server must error, not hang"
  );
  assert!(
    elapsed < Duration::from_secs(5),
    "download should fail fast via the configured timeout, but took {elapsed:?}"
  );
}

#[tokio::test]
async fn get_update_times_out() {
  let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
  let port = server.server_addr().to_ip().unwrap().port();
  std::thread::spawn(move || {
    for request in server.incoming_requests() {
      std::thread::sleep(Duration::from_secs(30));
      drop(request);
    }
  });

  let remote = Remote::builder()
    .base_url(format!("http://127.0.0.1:{port}"))
    .http(HttpOptions::new().timeout(150))
    .build()
    .unwrap();

  let start = Instant::now();
  let result = remote.get_update(None).await;
  let elapsed = start.elapsed();

  assert!(
    result.is_err(),
    "get_update against an unresponsive server must error, not hang"
  );
  assert!(
    elapsed < Duration::from_secs(5),
    "get_update should fail fast via the configured timeout, but took {elapsed:?}"
  );
}
