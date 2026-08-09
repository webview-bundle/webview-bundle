use crate::integrity::IntegrityAlgorithm;
#[cfg(feature = "signature-ed25519")]
use crate::remote::sfv::parse_string_dict;
use crate::remote::{BundleUpdate, Remote, Update};
#[cfg(feature = "signature-ed25519")]
use crate::signature::{Ed25519, SignatureAlgorithm, SignatureKey, SignatureKeySet};
use crate::testing::bundle::TestingBundle;
use crate::testing::bundle_collection::TestingBundleCollection;
use httpmock::{HttpMockRequest, HttpMockResponse, MockExt, MockServer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use twox_hash::XxHash64;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum TestingRemoteServerEndpoint {
  GetUpdate,
  Download,
}

#[derive(Default)]
pub struct TestingRemoteServerEndpointMocks(HashMap<TestingRemoteServerEndpoint, usize>);

impl TestingRemoteServerEndpointMocks {
  pub fn get_id(&self, endpoint: TestingRemoteServerEndpoint) -> usize {
    *self.0.get(&endpoint).unwrap()
  }

  fn insert(&mut self, endpoint: TestingRemoteServerEndpoint, id: usize) -> &mut Self {
    self.0.insert(endpoint, id);
    self
  }
}

#[derive(Default)]
struct CurrentVersions(HashMap<String, String>);

impl CurrentVersions {
  fn set(&mut self, name: impl Into<String>, version: impl Into<String>) -> &mut Self {
    self.0.insert(name.into(), version.into());
    self
  }

  fn unset(&mut self, name: impl Into<String>) -> &mut Self {
    self.0.remove(&name.into());
    self
  }

  fn unset_if_current(&mut self, name: impl Into<String>, version: impl Into<String>) -> &mut Self {
    let name = name.into();
    let version = version.into();
    if let Some(v) = self.0.get(&name)
      && v == &version
    {
      self.0.remove(&name);
    }
    self
  }

  /// The versions as a list ordered by bundle name, so a response built from them is
  /// byte-for-byte stable and its etag only changes when the versions themselves do.
  fn sorted(&self) -> Vec<(String, String)> {
    let mut versions = self
      .0
      .iter()
      .map(|(name, version)| (name.to_owned(), version.to_owned()))
      .collect::<Vec<_>>();
    versions.sort();
    versions
  }
}

type Bundles = Arc<Mutex<TestingBundleCollection>>;
type Versions = Arc<Mutex<CurrentVersions>>;
type ChannelVersions = Arc<Mutex<HashMap<String, CurrentVersions>>>;
#[cfg(feature = "signature-ed25519")]
type SignatureKeys = Arc<Mutex<HashMap<String, ed25519_dalek::SigningKey>>>;

#[non_exhaustive]
pub struct TestingRemoteServer {
  server: MockServer,
  bundles: Bundles,
  current_versions: Versions,
  channel_current_versions: ChannelVersions,
  #[cfg(feature = "signature-ed25519")]
  signature_keys: SignatureKeys,
  created_at: String,
  pub mocks: TestingRemoteServerEndpointMocks,
}

impl Default for TestingRemoteServer {
  fn default() -> Self {
    let mut instance = Self {
      server: MockServer::start(),
      bundles: Arc::new(Mutex::new(TestingBundleCollection::new())),
      current_versions: Default::default(),
      channel_current_versions: Default::default(),
      #[cfg(feature = "signature-ed25519")]
      signature_keys: Default::default(),
      created_at: "2026-01-01T00:00:00Z".to_owned(),
      mocks: Default::default(),
    };
    instance.init();
    instance
  }
}

impl TestingRemoteServer {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn remote(&self) -> anyhow::Result<Remote> {
    let remote = Remote::builder().base_url(self.base_url()).build()?;
    Ok(remote)
  }

  pub fn base_url(&self) -> String {
    format!("http://{}:{}", self.server.host(), self.server.port())
  }

  pub fn insert_bundle(&mut self, bundle: TestingBundle) -> bool {
    let inserted = {
      let mut bundles = self.bundles.lock().unwrap();
      bundles.insert(bundle)
    };
    inserted
  }

  pub fn remove_bundle(&mut self, bundle: TestingBundle) -> bool {
    let mut bundles = self.bundles.lock().unwrap();
    bundles.remove(bundle)
  }

  pub fn set_current_version(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self.current_versions.lock().unwrap().set(name, version);
    self
  }

  pub fn unset_current_version(&mut self, name: impl Into<String>) -> &mut Self {
    self.current_versions.lock().unwrap().unset(name);
    self
  }

  pub fn unset_current_version_if_current(
    &mut self,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self
      .current_versions
      .lock()
      .unwrap()
      .unset_if_current(name, version);
    self
  }

  pub fn set_channel_current_version(
    &mut self,
    channel: impl Into<String>,
    name: impl Into<String>,
    version: impl Into<String>,
  ) -> &mut Self {
    self
      .channel_current_versions
      .lock()
      .unwrap()
      .entry(channel.into())
      .or_default()
      .set(name, version);
    self
  }

  pub fn unset_channel_current_version(
    &mut self,
    channel: impl Into<String>,
    name: impl Into<String>,
  ) -> &mut Self {
    if let Some(versions) = self
      .channel_current_versions
      .lock()
      .unwrap()
      .get_mut(&channel.into())
    {
      versions.unset(name);
    }
    self
  }

  /// Registers the private key the server signs update responses with, for clients that
  /// ask for `key_id` through the `wvb-expect-signature` header.
  #[cfg(feature = "signature-ed25519")]
  pub fn insert_signature_key(&mut self, key_id: impl Into<String>, seed: [u8; 32]) -> &mut Self {
    self
      .signature_keys
      .lock()
      .unwrap()
      .insert(key_id.into(), ed25519_dalek::SigningKey::from_bytes(&seed));
    self
  }

  /// The public half of a registered key, ready to be handed to
  /// `RemoteGetUpdateOptions::expect_signature`.
  #[cfg(feature = "signature-ed25519")]
  pub fn signature_key_set(&self, key_id: &str) -> Option<SignatureKeySet> {
    let keys = self.signature_keys.lock().unwrap();
    let signing_key = keys.get(key_id)?;
    let key = Ed25519::from_public_key_bytes(&signing_key.verifying_key().to_bytes()).ok()?;
    Some(SignatureKeySet {
      id: key_id.to_owned(),
      key: SignatureKey::Ed25519(key),
    })
  }

  fn init(&mut self) {
    let bundles = Arc::clone(&self.bundles);
    let current_versions = Arc::clone(&self.current_versions);
    let channel_current_versions = Arc::clone(&self.channel_current_versions);
    #[cfg(feature = "signature-ed25519")]
    let signature_keys = Arc::clone(&self.signature_keys);
    let created_at = self.created_at.clone();

    let get_update = self.server.mock(|when, then| {
      when.method("GET").path_matches(r"^/update$");

      then.respond_with(move |req: &HttpMockRequest| {
        if header_of(req, "wvb-update-protocol-version").as_deref()
          != Some(crate::remote::UPDATE_PROTOCOL_VERSION)
        {
          return bad_request("\"wvb-update-protocol-version\" header must be \"1\"");
        }

        let channel = header_of(req, "wvb-update-channel");
        let versions = match &channel {
          Some(channel) => channel_current_versions
            .lock()
            .unwrap()
            .get(channel)
            .map(|x| x.sorted())
            .unwrap_or_default(),
          None => current_versions.lock().unwrap().sorted(),
        };

        let update = Update {
          id: update_id(channel.as_deref(), &versions),
          created_at: created_at.clone(),
          expires_at: None,
          runtime_version: crate::UPDATE_RUNTIME_VERSION,
          bundles: bundle_updates(&bundles, &versions),
          metadata: match &channel {
            Some(channel) => HashMap::from([("channel".to_owned(), channel.to_owned())]),
            None => HashMap::new(),
          },
        };
        let body = match serde_json::to_vec(&update) {
          Ok(body) => body,
          Err(e) => return bad_request(&e.to_string()),
        };
        let etag = etag_of(&body);

        #[cfg(feature = "signature-ed25519")]
        let signature = match header_of(req, "wvb-expect-signature") {
          Some(expect) => match sign_body(&signature_keys, &expect, &body) {
            Ok(signature) => Some(signature),
            Err(message) => return bad_request(&message),
          },
          None => None,
        };
        #[cfg(not(feature = "signature-ed25519"))]
        let signature: Option<String> = None;

        if header_of(req, "if-none-match").as_deref() == Some(etag.as_str()) {
          return HttpMockResponse::builder()
            .status(304)
            .header("etag", etag)
            .build();
        }

        let mut builder = HttpMockResponse::builder()
          .status(200)
          .header("content-type", "application/json")
          .header("etag", etag);
        if let Some(signature) = signature {
          builder = builder.header("wvb-signature", signature);
        }
        builder.body(body).build()
      });
    });

    let bundles = Arc::clone(&self.bundles);
    let download = self.server.mock(|when, then| {
      when
        .method("GET")
        .path_matches(r"^/bundles/([^/]+)/([^/]+)$");

      then.respond_with(move |req| {
        let bundle_name = req
          .uri()
          .path()
          .split('/')
          .nth(2)
          .map(String::from)
          .unwrap();
        let version = req
          .uri()
          .path()
          .split('/')
          .nth(3)
          .map(String::from)
          .unwrap();
        let collection = bundles.lock().unwrap();
        let bundle = if let Some(b) = collection.get(bundle_name, version) {
          b
        } else {
          return HttpMockResponse::builder().status(404).build();
        };
        HttpMockResponse::builder()
          .status(200)
          .header("content-type", "application/webview-bundle")
          .body(bundle.make_bundle_data().unwrap())
          .build()
      });
    });

    self
      .mocks
      .insert(TestingRemoteServerEndpoint::GetUpdate, get_update.id())
      .insert(TestingRemoteServerEndpoint::Download, download.id());
  }
}

fn header_of(req: &HttpMockRequest, name: &str) -> Option<String> {
  req
    .headers_vec()
    .iter()
    .find(|(key, _)| key.eq_ignore_ascii_case(name))
    .map(|(_, value)| value.to_owned())
}

fn bad_request(message: &str) -> HttpMockResponse {
  HttpMockResponse::builder()
    .status(400)
    .header("content-type", "application/json")
    .body(format!("{{\"message\":{}}}", escape_json(message)))
    .build()
}

fn escape_json(value: &str) -> String {
  serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

fn bundle_updates(bundles: &Bundles, versions: &[(String, String)]) -> Vec<BundleUpdate> {
  let collection = bundles.lock().unwrap();
  versions
    .iter()
    .filter_map(|(name, version)| {
      let bundle = collection.get(name.as_str(), version.as_str())?;
      let integrity = bundle.make_integrity(IntegrityAlgorithm::Sha256).ok()?;
      Some(BundleUpdate {
        name: name.to_owned(),
        version: version.to_owned(),
        download_url: None,
        integrity: Some(integrity.serialize()),
        metadata: None,
      })
    })
    .collect()
}

/// A uuid derived from the versions being served, so that it stays stable while they do
/// and is regenerated as soon as any of them changes.
fn update_id(channel: Option<&str>, versions: &[(String, String)]) -> String {
  let mut seed = channel.unwrap_or_default().to_owned();
  for (name, version) in versions {
    seed.push('\0');
    seed.push_str(name);
    seed.push('@');
    seed.push_str(version);
  }

  let mut bytes = [0u8; 16];
  bytes[..8].copy_from_slice(&XxHash64::oneshot(0, seed.as_bytes()).to_be_bytes());
  bytes[8..].copy_from_slice(&XxHash64::oneshot(1, seed.as_bytes()).to_be_bytes());
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  let hex = bytes
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
  format!(
    "{}-{}-{}-{}-{}",
    &hex[0..8],
    &hex[8..12],
    &hex[12..16],
    &hex[16..20],
    &hex[20..32]
  )
}

fn etag_of(body: &[u8]) -> String {
  format!("\"{:016x}\"", XxHash64::oneshot(0, body))
}

#[cfg(feature = "signature-ed25519")]
fn sign_body(keys: &SignatureKeys, expect: &str, body: &[u8]) -> Result<String, String> {
  use base64ct::{Base64, Encoding};
  use ed25519_dalek::Signer;

  let dict = parse_string_dict(expect)
    .ok_or_else(|| "\"wvb-expect-signature\" header is malformed".to_owned())?;
  let key_id = dict
    .get("key_id")
    .ok_or_else(|| "\"wvb-expect-signature\" header is missing \"key_id\"".to_owned())?;
  let alg = dict
    .get("alg")
    .ok_or_else(|| "\"wvb-expect-signature\" header is missing \"alg\"".to_owned())?;
  if alg != &SignatureAlgorithm::Ed25519.to_string() {
    return Err(format!("signature algorithm {alg:?} is not supported"));
  }

  let keys = keys.lock().unwrap();
  let signing_key = keys
    .get(key_id)
    .ok_or_else(|| format!("signature key {key_id:?} not found"))?;
  let sig = Base64::encode_string(&signing_key.sign(body).to_bytes());

  Ok(format!("key_id=\"{key_id}\", alg=\"{alg}\", sig=\"{sig}\""))
}
