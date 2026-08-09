use crate::remote::streaming::stream_to_file;
use crate::remote::{
  HttpOptions, RemoteConfig, RemoteError, RemoteGetUpdateOptions, RemoteUpdateResponse, Update,
  UpdateSignature,
};
#[cfg(feature = "signature")]
use crate::signature::SignatureVerifier;
use crate::util::cancellation::Cancellation;
use http::StatusCode;
use reqwest::header;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct RemoteBuilder {
  config: RemoteConfig,
}

impl RemoteBuilder {
  #[must_use]
  /// Set the base url of the remote server
  pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
    self.config.base_url = base_url.into();
    self
  }

  /// Set HTTP client options.
  pub fn http(mut self, http: HttpOptions) -> Self {
    self.config.http = Some(http);
    self
  }

  /// Set download progress callback.
  pub fn on_download<F>(mut self, on_download: F) -> Self
  where
    F: Fn(u64, Option<u64>, String) + Send + Sync + 'static,
  {
    self.config.on_download = Some(Arc::new(on_download));
    self
  }

  /// Build the remote client with the configured options.
  pub fn build(self) -> crate::Result<Remote> {
    if self.config.base_url.is_empty() {
      return Err(crate::Error::invalid_remote_config(
        "\"base_url\" is required",
      ));
    }
    if http::uri::Uri::from_str(&self.config.base_url).is_err() {
      return Err(crate::Error::invalid_remote_config(
        "\"base_url\" is invalid",
      ));
    }
    let http_options = self.config.http.clone().unwrap_or_default();
    let client_builder = http_options.apply(reqwest::ClientBuilder::new());
    let client = client_builder.build()?;
    Ok(Remote {
      config: self.config,
      client,
    })
  }
}

#[derive(Clone)]
pub struct Remote {
  pub(crate) config: RemoteConfig,
  client: reqwest::Client,
}

impl Remote {
  pub fn builder() -> RemoteBuilder {
    RemoteBuilder::default()
  }

  /// Gets update information for the remote server.
  pub async fn get_update(
    &self,
    options: Option<RemoteGetUpdateOptions>,
  ) -> crate::Result<Option<RemoteUpdateResponse>> {
    let endpoint = format!(
      "{}/update",
      self
        .config
        .base_url
        .strip_suffix('/')
        .unwrap_or(&self.config.base_url)
    );
    let mut req = self
      .client
      .get(endpoint)
      .header(header::ACCEPT, "application/json")
      .header(
        header::HeaderName::from_static("wvb-update-protocol-version"),
        crate::remote::UPDATE_PROTOCOL_VERSION,
      )
      .header(
        header::HeaderName::from_static("wvb-runtime-version"),
        crate::UPDATE_RUNTIME_VERSION.to_string(),
      );

    if let Some(options) = &options {
      if let Some(etag) = &options.etag {
        req = req.header(header::IF_NONE_MATCH, etag);
      }
      if let Some(channel) = &options.channel {
        req = req.header(
          header::HeaderName::from_static("wvb-update-channel"),
          channel,
        );
      }
      #[cfg(feature = "signature")]
      {
        if let Some(sig) = &options.expect_signature {
          let value = format!("key_id=\"{}\", alg=\"{}\"", sig.id, sig.algorithm());
          req = req.header(
            header::HeaderName::from_static("wvb-expect-signature"),
            value,
          );
        }
      }
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
      if resp.status() == StatusCode::NOT_MODIFIED {
        return Ok(None);
      }
      return Err(self.parse_err(resp).await);
    }

    let etag = resp
      .headers()
      .get(header::ETAG)
      .map(|x| {
        x.to_str()
          .map_err(|_| crate::Error::bad_remote_response("\"etag\" header is invalid format"))
      })
      .transpose()?
      .map(|x| x.to_string());
    let signature = match resp
      .headers()
      .get(header::HeaderName::from_static("wvb-signature"))
    {
      Some(value) => {
        let value = value.to_str().map_err(|_| {
          crate::Error::bad_remote_response("\"wvb-signature\" header is invalid format")
        })?;
        Some(value.parse::<UpdateSignature>()?)
      }
      None => None,
    };

    let bytes = resp.bytes().await?;

    #[cfg(feature = "signature")]
    {
      if let Some(options) = &options
        && let Some(sig) = &options.expect_signature
      {
        if let Some(sig_from_server) = &signature {
          sig.key.verify(&bytes, &sig_from_server.sig).await?;
        } else {
          return Err(crate::Error::expect_signature_not_found(&sig));
        }
      }
    }

    let update = serde_json::from_slice::<Update>(&bytes)?;
    let update_resp = RemoteUpdateResponse {
      update,
      etag,
      signature,
    };

    Ok(Some(update_resp))
  }

  /// Download bundle into given file path.
  pub async fn download(
    &self,
    url: impl Into<String>,
    filepath: &Path,
    cancellation: Option<Cancellation>,
  ) -> crate::Result<()> {
    let cancellation = cancellation.unwrap_or_default();

    let resp = cancellation
      .run_until_cancelled(self.client.get(url.into()).send())
      .await??;

    if !resp.status().is_success() {
      return Err(self.parse_err(resp).await);
    }

    stream_to_file(
      resp,
      filepath,
      Some(cancellation),
      self.config.on_download.clone(),
    )
    .await?;

    Ok(())
  }

  pub(crate) fn default_download_url(&self, bundle_name: &str, version: &str) -> String {
    format!("{}/bundles/{bundle_name}/{version}", self.config.base_url)
  }

  async fn parse_err(&self, resp: reqwest::Response) -> crate::Error {
    let status = resp.status();
    let message = match resp.text().await {
      Ok(text) => serde_json::from_str::<RemoteError>(&text)
        .ok()
        .and_then(|x| x.message)
        .or_else(|| {
          let text = text.trim();
          (!text.is_empty()).then(|| text.to_owned())
        }),
      Err(_) => None,
    };
    crate::Error::remote_http(status, message)
  }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
  use super::*;
  use crate::ErrorCode;
  use crate::integrity::IntegrityAlgorithm;
  use crate::remote::BundleUpdate;
  use crate::testing::{TempDir, TestingBundle, TestingRemoteServer};
  use httpmock::{HttpMockRequest, HttpMockResponse, MockServer};
  use std::collections::HashMap;
  use std::io::{Read, Write};
  use std::net::TcpListener;
  use std::sync::Mutex;

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  use crate::signature::{Ed25519, SignatureKey, SignatureKeySet};
  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  use base64ct::{Base64, Encoding};
  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  use ed25519_dalek::{Signer, SigningKey};

  const UPDATE_BODY: &str = r#"{"id":"u1","createdAt":"2026-08-08T00:00:00Z","runtimeVersion":1,"bundles":[{"name":"app","version":"1.2.3","downloadUrl":"https://cdn.example.com/app.wvb","integrity":"sha256-abc"}],"metadata":{"channel":"stable"}}"#;

  type Request = (String, String, Vec<(String, String)>);
  type Captured = Arc<Mutex<Vec<Request>>>;
  type Progress = Arc<Mutex<Vec<(u64, Option<u64>)>>>;

  fn mock_server(
    status: u16,
    headers: Vec<(&str, &str)>,
    body: impl Into<Vec<u8>>,
  ) -> (MockServer, Captured) {
    let server = MockServer::start();
    let captured: Captured = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&captured);
    let headers = headers
      .into_iter()
      .map(|(name, value)| (name.to_owned(), value.to_owned()))
      .collect::<Vec<_>>();
    let body = body.into();

    server.mock(|when, then| {
      when.any_request();
      then.respond_with(move |req: &HttpMockRequest| {
        sink.lock().unwrap().push((
          req.method_str().to_owned(),
          req.uri().path().to_owned(),
          req.headers_vec().clone(),
        ));
        HttpMockResponse::builder()
          .status(status)
          .headers(headers.clone())
          .body(body.clone())
          .build()
      });
    });

    (server, captured)
  }

  fn raw_server(response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
      let (mut stream, _) = listener.accept().unwrap();
      let mut head = vec![];
      let mut buf = [0u8; 1024];
      while !head.windows(4).any(|x| x == b"\r\n\r\n") {
        match stream.read(&mut buf) {
          Ok(0) | Err(_) => break,
          Ok(read) => head.extend_from_slice(&buf[..read]),
        }
      }
      let _ = stream.write_all(&response);
      let _ = stream.flush();
    });
    format!("http://{addr}")
  }

  fn remote(base_url: impl Into<String>) -> Remote {
    Remote::builder().base_url(base_url).build().unwrap()
  }

  fn remote_with_progress(base_url: impl Into<String>) -> (Remote, Progress) {
    let progress: Progress = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&progress);
    let remote = Remote::builder()
      .base_url(base_url)
      .on_download(move |downloaded, total, _| sink.lock().unwrap().push((downloaded, total)))
      .build()
      .unwrap();
    (remote, progress)
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  fn key_set(id: &str) -> SignatureKeySet {
    let key = Ed25519::from_public_key_bytes(&signing_key().verifying_key().to_bytes()).unwrap();
    SignatureKeySet {
      id: id.to_owned(),
      key: SignatureKey::Ed25519(key),
    }
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  fn sign(message: &[u8]) -> String {
    Base64::encode_string(&signing_key().sign(message).to_bytes())
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  fn signature_header(key_id: &str, sig: &str) -> String {
    format!("key_id=\"{key_id}\", alg=\"ed25519\", sig=\"{sig}\"")
  }

  fn captured_requests(captured: &Captured) -> Vec<Request> {
    captured.lock().unwrap().clone()
  }

  fn testing_bundle(name: &str, version: &str) -> TestingBundle {
    TestingBundle::new(name, version)
  }

  fn testing_server() -> TestingRemoteServer {
    let mut server = TestingRemoteServer::new();
    server.insert_bundle(testing_bundle("app", "1.0.0"));
    server.insert_bundle(testing_bundle("app", "1.2.3"));
    server.insert_bundle(testing_bundle("admin", "0.1.0"));
    server.set_current_version("app", "1.0.0");
    server
  }

  fn served_bundles(update: &RemoteUpdateResponse) -> Vec<(&str, &str)> {
    update
      .update
      .bundles
      .iter()
      .map(|x| (x.name.as_str(), x.version.as_str()))
      .collect()
  }

  fn header_of<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
      .2
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  #[track_caller]
  fn remote_http(err: crate::Error) -> (u16, Option<String>) {
    match err {
      crate::Error::RemoteHttp { status, message } => (status, message),
      _ => panic!("expected remote http error, got {err:?}"),
    }
  }

  #[track_caller]
  fn bad_remote_response(err: crate::Error) -> String {
    match err {
      crate::Error::BadRemoteResponse(message) => message,
      _ => panic!("expected bad remote response error, got {err:?}"),
    }
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[track_caller]
  fn expect_signature_not_found(err: crate::Error) -> (String, String) {
    match err {
      crate::Error::ExpectSignatureNotFound { key_id, alg } => (key_id, alg),
      _ => panic!("expected expect signature not found error, got {err:?}"),
    }
  }

  fn expected_update() -> Update {
    Update {
      id: "u1".to_owned(),
      created_at: "2026-08-08T00:00:00Z".to_owned(),
      expires_at: None,
      runtime_version: 1,
      bundles: vec![BundleUpdate {
        name: "app".to_owned(),
        version: "1.2.3".to_owned(),
        download_url: Some("https://cdn.example.com/app.wvb".to_owned()),
        integrity: Some("sha256-abc".to_owned()),
        metadata: None,
      }],
      metadata: HashMap::from([("channel".to_owned(), "stable".to_owned())]),
    }
  }

  fn expected_update_resp() -> RemoteUpdateResponse {
    RemoteUpdateResponse {
      update: expected_update(),
      etag: None,
      signature: None,
    }
  }

  #[tokio::test]
  async fn requests_update_endpoint() {
    let (server, captured) = mock_server(200, vec![], UPDATE_BODY);

    remote(server.base_url()).get_update(None).await.unwrap();

    let requests = captured_requests(&captured);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/update");
    assert_eq!(header_of(&requests[0], "accept"), Some("application/json"));
    assert_eq!(
      header_of(&requests[0], "wvb-update-protocol-version"),
      Some("1")
    );
    assert_eq!(header_of(&requests[0], "wvb-runtime-version"), Some("1"));
    assert_eq!(header_of(&requests[0], "if-none-match"), None);
    assert_eq!(header_of(&requests[0], "wvb-update-channel"), None);
    assert_eq!(header_of(&requests[0], "wvb-expect-signature"), None);
  }

  #[tokio::test]
  async fn resolves_endpoint_from_base_url() {
    let (server, captured) = mock_server(200, vec![], UPDATE_BODY);

    remote(format!("{}/", server.base_url()))
      .get_update(None)
      .await
      .unwrap();
    remote(format!("{}/api", server.base_url()))
      .get_update(None)
      .await
      .unwrap();
    remote(format!("{}/api/", server.base_url()))
      .get_update(None)
      .await
      .unwrap();

    let requests = captured_requests(&captured);
    assert_eq!(
      requests.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
      vec!["/update", "/api/update", "/api/update"]
    );
  }

  #[tokio::test]
  async fn sends_optional_headers() {
    let (server, captured) = mock_server(200, vec![], UPDATE_BODY);
    let options = RemoteGetUpdateOptions::default()
      .etag("etag-1")
      .channel("beta");

    remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap();

    let requests = captured_requests(&captured);
    assert_eq!(header_of(&requests[0], "if-none-match"), Some("etag-1"));
    assert_eq!(header_of(&requests[0], "wvb-update-channel"), Some("beta"));
    assert_eq!(header_of(&requests[0], "wvb-expect-signature"), None);
  }

  #[tokio::test]
  async fn parses_update() {
    let (server, _) = mock_server(200, vec![], UPDATE_BODY);

    let update = remote(server.base_url()).get_update(None).await.unwrap();

    assert_eq!(update, Some(expected_update_resp()));
  }

  #[tokio::test]
  async fn parses_signature_header_without_verifying() {
    let (server, _) = mock_server(
      200,
      vec![(
        "wvb-signature",
        "key_id=\"default\", alg=\"ed25519\", sig=\"c2ln\"",
      )],
      UPDATE_BODY,
    );

    let update = remote(server.base_url())
      .get_update(None)
      .await
      .unwrap()
      .unwrap();

    assert_eq!(
      update.signature,
      Some(UpdateSignature {
        key_id: "default".to_owned(),
        sig: "c2ln".to_owned(),
        alg: "ed25519".to_owned(),
      })
    );
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn sends_expect_signature_header() {
    let header = signature_header("default", &sign(UPDATE_BODY.as_bytes()));
    let (server, captured) =
      mock_server(200, vec![("wvb-signature", header.as_str())], UPDATE_BODY);
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set("default"));

    remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap();

    let requests = captured_requests(&captured);
    assert_eq!(
      header_of(&requests[0], "wvb-expect-signature"),
      Some("key_id=\"default\", alg=\"ed25519\"")
    );
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn verifies_signature_over_the_response_body() {
    let mut server = testing_server();
    server.insert_signature_key("default", [7u8; 32]);
    let options = RemoteGetUpdateOptions::default()
      .expect_signature(server.signature_key_set("default").unwrap());

    let update = server
      .remote()
      .unwrap()
      .get_update(Some(options))
      .await
      .unwrap()
      .unwrap();

    assert_eq!(served_bundles(&update), vec![("app", "1.0.0")]);
    let signature = update.signature.unwrap();
    assert_eq!(signature.key_id, "default");
    assert_eq!(signature.alg, "ed25519");
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn errors_when_the_server_has_no_such_signature_key() {
    let mut server = testing_server();
    server.insert_signature_key("default", [7u8; 32]);
    let mut key_set = server.signature_key_set("default").unwrap();
    key_set.id = "rotated".to_owned();
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set);

    let err = server
      .remote()
      .unwrap()
      .get_update(Some(options))
      .await
      .unwrap_err();

    let (status, message) = remote_http(err);
    assert_eq!(status, 400);
    assert!(message.unwrap().contains("rotated"));
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn rejects_signature_over_other_bytes() {
    let header = signature_header("default", &sign(b"other bytes"));
    let (server, _) = mock_server(200, vec![("wvb-signature", header.as_str())], UPDATE_BODY);
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set("default"));

    let err = remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidSignature);
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn rejects_tampered_body() {
    let tampered = UPDATE_BODY.replace("1.2.3", "6.6.6");
    let header = signature_header("default", &sign(UPDATE_BODY.as_bytes()));
    let (server, _) = mock_server(200, vec![("wvb-signature", header.as_str())], tampered);
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set("default"));

    let err = remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidSignature);
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn rejects_response_without_expected_signature_header() {
    let (server, _) = mock_server(200, vec![], UPDATE_BODY);
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set("2026-08"));

    let err = remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap_err();

    assert_eq!(
      expect_signature_not_found(err),
      ("2026-08".to_owned(), "ed25519".to_owned())
    );
  }

  #[cfg(all(feature = "signature", feature = "signature-ed25519"))]
  #[tokio::test]
  async fn verifies_before_parsing_the_body() {
    let header = signature_header("default", &sign(b"other bytes"));
    let (server, _) = mock_server(200, vec![("wvb-signature", header.as_str())], "not json");
    let options = RemoteGetUpdateOptions::default().expect_signature(key_set("default"));

    let err = remote(server.base_url())
      .get_update(Some(options))
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidSignature);
  }

  #[tokio::test]
  async fn rejects_malformed_signature_header_before_body() {
    let (server, _) = mock_server(200, vec![("wvb-signature", "garbage")], "not json");

    let err = remote(server.base_url())
      .get_update(None)
      .await
      .unwrap_err();

    let message = bad_remote_response(err);
    assert!(message.contains("malformed"), "{message}");
  }

  #[tokio::test]
  async fn rejects_non_ascii_signature_header() {
    let mut response = b"HTTP/1.1 200 OK\r\nconnection: close\r\nwvb-signature: ".to_vec();
    response.extend_from_slice(&[0xc3, 0x28]);
    response
      .extend_from_slice(format!("\r\ncontent-length: {}\r\n\r\n", UPDATE_BODY.len()).as_bytes());
    response.extend_from_slice(UPDATE_BODY.as_bytes());

    let err = remote(raw_server(response))
      .get_update(None)
      .await
      .unwrap_err();

    let message = bad_remote_response(err);
    assert!(message.contains("invalid"), "{message}");
  }

  #[tokio::test]
  async fn serves_current_versions() {
    let server = testing_server();

    let update = server
      .remote()
      .unwrap()
      .get_update(None)
      .await
      .unwrap()
      .unwrap();

    assert_eq!(update.update.runtime_version, 1);
    assert_eq!(served_bundles(&update), vec![("app", "1.0.0")]);
    assert_eq!(
      update.update.bundles[0].integrity,
      Some(
        testing_bundle("app", "1.0.0")
          .make_integrity(IntegrityAlgorithm::Sha256)
          .unwrap()
          .serialize()
      )
    );
  }

  #[tokio::test]
  async fn returns_none_when_not_modified() {
    let server = testing_server();
    let remote = server.remote().unwrap();
    let etag = remote
      .get_update(None)
      .await
      .unwrap()
      .unwrap()
      .etag
      .unwrap();

    let options = RemoteGetUpdateOptions::default().etag(&etag);
    assert_eq!(remote.get_update(Some(options)).await.unwrap(), None);

    let options = RemoteGetUpdateOptions::default().etag("\"stale\"");
    assert!(remote.get_update(Some(options)).await.unwrap().is_some());
  }

  #[tokio::test]
  async fn etag_changes_when_served_versions_change() {
    let mut server = testing_server();
    let remote = server.remote().unwrap();
    let first = remote.get_update(None).await.unwrap().unwrap();

    server.set_current_version("app", "1.2.3");
    let options = RemoteGetUpdateOptions::default().etag(first.etag.as_ref().unwrap());
    let second = remote.get_update(Some(options)).await.unwrap().unwrap();

    assert_ne!(second.etag, first.etag);
    assert_ne!(second.update.id, first.update.id);
    assert_eq!(served_bundles(&second), vec![("app", "1.2.3")]);
  }

  #[tokio::test]
  async fn serves_channel_versions_only() {
    let mut server = testing_server();
    server.set_channel_current_version("beta", "admin", "0.1.0");
    let remote = server.remote().unwrap();

    let options = RemoteGetUpdateOptions::default().channel("beta");
    let update = remote.get_update(Some(options)).await.unwrap().unwrap();

    assert_eq!(served_bundles(&update), vec![("admin", "0.1.0")]);
    assert_eq!(
      update.update.metadata.get("channel").map(String::as_str),
      Some("beta")
    );
  }

  #[tokio::test]
  async fn errors_on_invalid_body() {
    let (server, _) = mock_server(200, vec![], "not json");

    let err = remote(server.base_url())
      .get_update(None)
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::SerdeJson);
  }

  #[tokio::test]
  async fn reads_error_message_from_json_body() {
    let (server, _) = mock_server(
      500,
      vec![("content-type", "application/json")],
      r#"{"message":"boom"}"#,
    );

    let err = remote(server.base_url())
      .get_update(None)
      .await
      .unwrap_err();

    assert_eq!(remote_http(err), (500, Some("boom".to_owned())));
  }

  #[tokio::test]
  async fn falls_back_to_body_text_for_error_message() {
    let text = {
      let (server, _) = mock_server(400, vec![], "  boom  ");
      remote(server.base_url())
        .get_update(None)
        .await
        .unwrap_err()
    };
    assert_eq!(remote_http(text), (400, Some("boom".to_owned())));

    let json_without_message = {
      let (server, _) = mock_server(
        400,
        vec![("content-type", "application/json")],
        r#"{"error":"nope"}"#,
      );
      remote(server.base_url())
        .get_update(None)
        .await
        .unwrap_err()
    };
    assert_eq!(
      remote_http(json_without_message),
      (400, Some(r#"{"error":"nope"}"#.to_owned()))
    );
  }

  #[tokio::test]
  async fn downloads_to_file() {
    let (server, captured) = mock_server(200, vec![], b"bundle data".to_vec());
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    remote(server.base_url())
      .download(server.url("/app.wvb"), &filepath, None)
      .await
      .unwrap();

    assert_eq!(tokio::fs::read(&filepath).await.unwrap(), b"bundle data");
    let requests = captured_requests(&captured);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/app.wvb");
  }

  #[tokio::test]
  async fn downloads_bundle_from_the_default_url() {
    let server = testing_server();
    let remote = server.remote().unwrap();
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    remote
      .download(remote.default_download_url("app", "1.0.0"), &filepath, None)
      .await
      .unwrap();

    assert_eq!(
      tokio::fs::read(&filepath).await.unwrap(),
      testing_bundle("app", "1.0.0").make_bundle_data().unwrap()
    );
  }

  #[tokio::test]
  async fn download_errors_when_bundle_is_not_served() {
    let server = testing_server();
    let remote = server.remote().unwrap();
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    let err = remote
      .download(remote.default_download_url("app", "9.9.9"), &filepath, None)
      .await
      .unwrap_err();

    assert_eq!(remote_http(err), (404, None));
    assert!(!filepath.exists());
  }

  #[tokio::test]
  async fn downloads_without_update_headers() {
    let (server, captured) = mock_server(200, vec![], b"bundle data".to_vec());
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    remote(server.base_url())
      .download(server.url("/app.wvb"), &filepath, None)
      .await
      .unwrap();

    let requests = captured_requests(&captured);
    assert!(header_of(&requests[0], "host").is_some());
    assert!(
      requests[0]
        .2
        .iter()
        .all(|(key, _)| !key.starts_with("wvb-")),
      "{:?}",
      requests[0].2
    );
    assert_ne!(header_of(&requests[0], "accept"), Some("application/json"));
  }

  #[tokio::test]
  async fn downloads_from_url_outside_base_url() {
    let (server, _) = mock_server(200, vec![], b"bundle data".to_vec());
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");

    remote("http://127.0.0.1:1")
      .download(server.url("/app.wvb"), &filepath, None)
      .await
      .unwrap();

    assert_eq!(tokio::fs::read(&filepath).await.unwrap(), b"bundle data");
  }

  #[tokio::test]
  async fn download_errors_on_failed_status() {
    let (server, _) = mock_server(404, vec![], r#"{"message":"missing"}"#);
    let temp = TempDir::new();
    let filepath = temp.dir().join("nested").join("app.wvb");

    let err = remote(server.base_url())
      .download(server.url("/app.wvb"), &filepath, None)
      .await
      .unwrap_err();

    assert_eq!(remote_http(err), (404, Some("missing".to_owned())));
    assert!(!filepath.exists());
    assert!(!filepath.parent().unwrap().exists());
  }

  #[tokio::test]
  async fn reports_download_progress() {
    let (server, _) = mock_server(200, vec![], b"bundle data".to_vec());
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let (remote, progress) = remote_with_progress(server.base_url());

    remote
      .download(server.url("/app.wvb"), &filepath, None)
      .await
      .unwrap();

    let progress = progress.lock().unwrap().clone();
    assert!(!progress.is_empty());
    assert!(progress.iter().all(|(_, total)| *total == Some(11)));
    assert_eq!(progress.last().unwrap().0, 11);
  }

  #[tokio::test]
  async fn download_cancelled_already() {
    let (server, _) = mock_server(200, vec![], b"bundle data".to_vec());
    let temp = TempDir::new();
    let filepath = temp.dir().join("app.wvb");
    let cancellation = Cancellation::new();
    cancellation.cancel();

    let err = remote(server.base_url())
      .download(server.url("/app.wvb"), &filepath, Some(cancellation))
      .await
      .unwrap_err();

    assert_eq!(err.code(), ErrorCode::Cancelled);
  }
}
