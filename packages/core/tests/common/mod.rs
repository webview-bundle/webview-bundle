use http::Request;

/// A `GET` request with an empty body.
pub fn get(uri: &str) -> Request<Vec<u8>> {
  Request::builder()
    .uri(uri)
    .method("GET")
    .body(vec![])
    .unwrap()
}
