use std::time::{Duration, Instant};
use wvb::remote::{HttpOptions, Remote};

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

  let remote = Remote::builder()
    .endpoint(format!("http://127.0.0.1:{port}"))
    .http(HttpOptions::new().timeout(150))
    .build()
    .unwrap();

  let start = Instant::now();
  let result = remote.download("app", None).await;
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
