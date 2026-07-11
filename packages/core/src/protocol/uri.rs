use http::Uri;
use std::sync::Arc;

pub type UriBundleResolveFn = dyn Fn(&Uri) -> Option<String> + Send + Sync + 'static;

#[derive(Clone, Copy, Debug, Default)]
pub enum HostnameSegment {
  /// First segment of hostname. (e.g. `app.wvb` -> `app`, `app.mydomain.com` -> `app`)
  #[default]
  First,
  /// Resolve full text.
  Full,
  /// Strip last segment. (e.g. `app.wvb` -> `app`, `app.mydomain` -> `app.mydomain`)
  StripSuffix,
  Nth(usize),
}

#[non_exhaustive]
#[derive(Clone)]
pub enum UriBundleResolver {
  /// Resolve bundle from uri hostname.
  /// Default to first hostname segment.
  Hostname {
    segment: HostnameSegment,
    allow_wvb_suffix_only: bool,
  },
  /// Resolve bundle from uri pathname.
  /// Can specify which segment to use (default: 0)
  Pathname {
    segment_index: usize,
  },
  Custom(Arc<UriBundleResolveFn>),
}

impl UriBundleResolver {
  /// Resolve the bundle name from the URI host.
  ///
  /// * `segment` - which part of the host to use (default: [`HostnameSegment::First`]).
  /// * `allow_wvb_suffix_only` - only resolve hosts ending in `.wvb` (default: `false`).
  pub fn hostname(segment: Option<HostnameSegment>, allow_wvb_suffix_only: Option<bool>) -> Self {
    Self::Hostname {
      segment: segment.unwrap_or_default(),
      allow_wvb_suffix_only: allow_wvb_suffix_only.unwrap_or(false),
    }
  }

  /// Resolve the bundle name from a path segment, 0-based over non-empty segments
  /// (default: `0`). e.g. `app://_/my-app/index.html` with index `0` -> `my-app`.
  pub fn pathname(segment_index: Option<usize>) -> Self {
    Self::Pathname {
      segment_index: segment_index.unwrap_or(0),
    }
  }

  /// Resolve the bundle name with a custom closure.
  pub fn custom<F>(resolve_fn: F) -> Self
  where
    F: Fn(&Uri) -> Option<String> + Send + Sync + 'static,
  {
    Self::Custom(Arc::new(resolve_fn))
  }

  pub fn resolve(&self, uri: &Uri) -> Option<String> {
    match self {
      Self::Hostname {
        segment,
        allow_wvb_suffix_only,
      } => {
        let host = uri.host()?;
        let host = host.strip_suffix('.').unwrap_or(host); // FQDN
        let host = host.to_ascii_lowercase();
        let has_wvb_suffix = host
          .rsplit_once('.')
          .is_some_and(|(_, ext)| ext == crate::consts::EXTENSION);
        if *allow_wvb_suffix_only && !has_wvb_suffix {
          return None;
        }
        let name = match segment {
          HostnameSegment::First => host.split('.').next().unwrap_or_default().to_owned(),
          HostnameSegment::Full => host.to_owned(),
          HostnameSegment::StripSuffix => host
            .rsplit_once('.')
            .map_or(host.to_owned(), |(h, _)| h.to_owned()),
          HostnameSegment::Nth(n) => host.split('.').nth(*n).unwrap_or_default().to_owned(),
        };
        (!name.is_empty()).then_some(name)
      }
      Self::Pathname { segment_index } => uri
        .path()
        .split("/")
        .filter(|s| !s.is_empty())
        .nth(*segment_index)
        .map(|x| x.to_string()),
      Self::Custom(resolve_fn) => resolve_fn(uri),
    }
  }
}

impl Default for UriBundleResolver {
  fn default() -> Self {
    Self::hostname(None, None)
  }
}

pub type UriPathResolveFn = dyn Fn(&Uri) -> String + Send + Sync + 'static;

#[non_exhaustive]
#[derive(Clone, Default)]
pub enum UriPathResolver {
  Exact,
  #[default]
  DirectoryIndex,
  HtmlExtension,
  Custom(Arc<UriPathResolveFn>),
}

impl UriPathResolver {
  /// Use the URI path as-is (only percent-decoded).
  pub fn exact() -> Self {
    Self::Exact
  }

  /// Directory index: `/` -> `/index.html`, `/about` -> `/about/index.html`.
  /// (static-site / MPA style; e.g. Astro `format: 'directory'`, Next `trailingSlash: true`)
  pub fn directory_index() -> Self {
    Self::DirectoryIndex
  }

  /// `.html` extension: `/` -> `/index.html`, `/about` -> `/about.html`.
  /// (flat-file style; e.g. Astro `format: 'file'`, GitHub Pages, Next `trailingSlash: false`)
  pub fn html_extension() -> Self {
    Self::HtmlExtension
  }

  /// Resolve the path with a custom closure.
  pub fn custom<F>(resolve_fn: F) -> Self
  where
    F: Fn(&Uri) -> String + Send + Sync + 'static,
  {
    Self::Custom(Arc::new(resolve_fn))
  }

  pub fn resolve(&self, uri: &Uri) -> String {
    // Last path segment has no `.`: looks like a route, not a file.
    let extensionless = |p: &str| {
      p.rsplit('/')
        .next()
        .is_some_and(|s| !s.is_empty() && !s.contains('.'))
    };
    match self {
      Self::Custom(resolve_fn) => resolve_fn(uri),
      Self::Exact => decode_path(&uri.path()),
      // `/` -> `/index.html`, `/about` -> `/about/index.html`
      Self::DirectoryIndex => {
        let mut path = decode_path(&uri.path());
        if path.is_empty() {
          path.push('/');
        }
        if path.ends_with('/') {
          path.push_str("index.html");
        } else if extensionless(&path) {
          path.push_str("/index.html");
        }
        path
      }
      // `/` -> `/index.html`, `/about` -> `/about.html`
      Self::HtmlExtension => {
        let mut path = decode_path(&uri.path());
        if path.is_empty() || path == "/" {
          return "/index.html".to_owned();
        }
        if path.ends_with('/') {
          path.pop();
        }
        if extensionless(&path) {
          path.push_str(".html");
        }
        path
      }
    }
  }
}

fn decode_path(path: &str) -> String {
  percent_encoding::percent_decode(path.as_bytes())
    .decode_utf8_lossy()
    .into_owned()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn uri(s: &str) -> Uri {
    s.parse().unwrap()
  }

  #[test]
  fn bundle_hostname_segments() {
    let first = UriBundleResolver::hostname(Some(HostnameSegment::First), None);
    assert_eq!(
      first.resolve(&uri("https://app.wvb/index.html")).as_deref(),
      Some("app")
    );
    assert_eq!(
      first.resolve(&uri("https://app.mydomain.com/")).as_deref(),
      Some("app")
    );
    assert_eq!(
      first.resolve(&uri("https://APP.WVB/")).as_deref(),
      Some("app")
    ); // case-insensitive

    let full = UriBundleResolver::hostname(Some(HostnameSegment::Full), None);
    assert_eq!(
      full.resolve(&uri("https://app.wvb/")).as_deref(),
      Some("app.wvb")
    );

    let strip = UriBundleResolver::hostname(Some(HostnameSegment::StripSuffix), None);
    assert_eq!(
      strip.resolve(&uri("https://app.wvb/")).as_deref(),
      Some("app")
    );
    assert_eq!(
      strip.resolve(&uri("https://a.b.wvb/")).as_deref(),
      Some("a.b")
    );

    let nth = UriBundleResolver::hostname(Some(HostnameSegment::Nth(1)), None);
    assert_eq!(nth.resolve(&uri("https://a.b.c/")).as_deref(), Some("b"));
  }

  #[test]
  fn bundle_allow_wvb_suffix_only() {
    let r = UriBundleResolver::hostname(None, Some(true));
    assert_eq!(r.resolve(&uri("https://app.wvb/")).as_deref(), Some("app"));
    assert_eq!(r.resolve(&uri("https://evil.com/")), None);
    assert_eq!(r.resolve(&uri("https://app/")), None); // no `.wvb` suffix
  }

  #[test]
  fn bundle_pathname_custom_default() {
    assert_eq!(
      UriBundleResolver::pathname(Some(0))
        .resolve(&uri("https://x/my-app/index.html"))
        .as_deref(),
      Some("my-app")
    );
    assert_eq!(
      UriBundleResolver::pathname(Some(1))
        .resolve(&uri("https://x/my-app/index.html"))
        .as_deref(),
      Some("index.html")
    );
    let custom = UriBundleResolver::custom(|_| Some("fixed".to_owned()));
    assert_eq!(
      custom.resolve(&uri("https://any/")).as_deref(),
      Some("fixed")
    );
    // Default: first hostname segment.
    assert_eq!(
      UriBundleResolver::default()
        .resolve(&uri("https://app.wvb/"))
        .as_deref(),
      Some("app")
    );
  }

  #[test]
  fn path_exact() {
    let r = UriPathResolver::exact();
    assert_eq!(r.resolve(&uri("https://x/about")), "/about");
    assert_eq!(r.resolve(&uri("https://x/")), "/");
  }

  #[test]
  fn path_directory_index() {
    let r = UriPathResolver::directory_index();
    assert_eq!(r.resolve(&uri("https://x/")), "/index.html");
    assert_eq!(r.resolve(&uri("https://x/about")), "/about/index.html");
    assert_eq!(r.resolve(&uri("https://x/about/")), "/about/index.html");
    assert_eq!(r.resolve(&uri("https://x/assets/app.js")), "/assets/app.js");
  }

  #[test]
  fn path_html_extension() {
    let r = UriPathResolver::html_extension();
    assert_eq!(r.resolve(&uri("https://x/")), "/index.html");
    assert_eq!(r.resolve(&uri("https://x/about")), "/about.html");
    assert_eq!(r.resolve(&uri("https://x/about/")), "/about.html");
    assert_eq!(r.resolve(&uri("https://x/assets/app.js")), "/assets/app.js");
  }

  #[test]
  fn path_custom_default() {
    let custom = UriPathResolver::custom(|_| "/fixed.html".to_owned());
    assert_eq!(custom.resolve(&uri("https://x/whatever")), "/fixed.html");
    // Default: directory index.
    assert_eq!(
      UriPathResolver::default().resolve(&uri("https://x/about")),
      "/about/index.html"
    );
  }
}
