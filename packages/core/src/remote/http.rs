use reqwest::header::HeaderMap;

/// Default total request timeout (milliseconds)
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug, Clone, Default)]
pub struct HttpOptions {
  pub default_headers: Option<HeaderMap>,
  pub user_agent: Option<String>,
  pub timeout: Option<u64>,
  pub read_timeout: Option<u64>,
  pub connect_timeout: Option<u64>,
  pub pool_idle_timeout: Option<u64>,
  pub pool_max_idle_per_host: Option<usize>,
  pub referer: Option<bool>,
  pub tcp_nodelay: Option<bool>,
}

impl HttpOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn default_headers(mut self, headers: HeaderMap) -> Self {
    self.default_headers = Some(headers);
    self
  }

  pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
    self.user_agent = Some(user_agent.into());
    self
  }

  pub fn timeout(mut self, timeout: u64) -> Self {
    self.timeout = Some(timeout);
    self
  }

  pub fn read_timeout(mut self, read_timeout: u64) -> Self {
    self.read_timeout = Some(read_timeout);
    self
  }

  pub fn connect_timeout(mut self, connect_timeout: u64) -> Self {
    self.connect_timeout = Some(connect_timeout);
    self
  }

  pub fn pool_idle_timeout(mut self, pool_idle_timeout: u64) -> Self {
    self.pool_idle_timeout = Some(pool_idle_timeout);
    self
  }

  pub fn pool_max_idle_per_host(mut self, pool_max_idle_per_host: usize) -> Self {
    self.pool_max_idle_per_host = Some(pool_max_idle_per_host);
    self
  }

  pub fn referer(mut self, referer: bool) -> Self {
    self.referer = Some(referer);
    self
  }

  pub fn tcp_nodelay(mut self, tcp_nodelay: bool) -> Self {
    self.tcp_nodelay = Some(tcp_nodelay);
    self
  }

  pub(crate) fn apply(&self, mut http: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    if let Some(default_headers) = self.default_headers.as_ref() {
      http = http.default_headers(default_headers.clone());
    }
    if let Some(user_agent) = self.user_agent.as_ref() {
      http = http.user_agent(user_agent);
    }
    let timeout = self.timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS);
    http = http.timeout(std::time::Duration::from_millis(timeout));
    if let Some(pool_idle_timeout) = self.pool_idle_timeout {
      http = http.pool_idle_timeout(std::time::Duration::from_millis(pool_idle_timeout));
    }
    if let Some(pool_max_idle_per_host) = self.pool_max_idle_per_host {
      http = http.pool_max_idle_per_host(pool_max_idle_per_host);
    }
    if let Some(referer) = self.referer {
      http = http.referer(referer);
    }
    if let Some(tcp_nodelay) = self.tcp_nodelay {
      http = http.tcp_nodelay(tcp_nodelay);
    }
    http
  }
}
