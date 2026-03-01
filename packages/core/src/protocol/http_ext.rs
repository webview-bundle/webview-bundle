pub(crate) trait HttpHeadersTracingInfo {
  fn tracing_info(&self) -> String;
}

impl HttpHeadersTracingInfo for http::HeaderMap {
  fn tracing_info(&self) -> String {
    self
      .iter()
      .map(|(k, v)| format!("{}={}", k, String::from_utf8_lossy(v.as_bytes())))
      .collect::<Vec<_>>()
      .join(", ")
  }
}
