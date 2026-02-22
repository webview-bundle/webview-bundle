use crate::protocol::uri::{DefaultUriResolver, UriResolver};
use crate::source::BundleSource;
use async_trait::async_trait;
use http::{HeaderValue, Method, Request, Response, StatusCode, header};
use http_range::HttpRange;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

/// Protocol handler for serving files from bundle sources.
///
/// `BundleProtocol` implements the `Protocol` trait to serve web resources from
/// `.wvb` bundle files stored in a `BundleSource`. It supports:
///
/// - GET and HEAD HTTP methods
/// - HTTP Range requests for streaming large files (video, audio)
/// - Content-Type and custom HTTP headers from bundle index
/// - Custom URI resolution for flexible URL-to-bundle mapping
///
/// # URI Format
///
/// By default, URIs are expected in the format:
///
/// ```text
/// scheme://bundle_name/path/to/file
/// ```
///
/// For example:
/// - `bundle://app/index.html` → bundle "app", file "/index.html"
/// - `app://myapp/assets/logo.png` → bundle "myapp", file "/assets/logo.png"
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "protocol")]
/// # async {
/// use wvb::protocol::{BundleProtocol, Protocol};
/// use wvb::source::BundleSource;
/// use std::sync::Arc;
///
/// let source = BundleSource::builder()
///     .builtin_dir("./bundles")
///     .remote_dir("./remote")
///     .build();
///
/// let protocol = BundleProtocol::new(Arc::new(source));
///
/// // Serve index.html from "app" bundle
/// let request = http::Request::builder()
///     .uri("bundle://app/index.html")
///     .method("GET")
///     .body(vec![])
///     .unwrap();
///
/// let response = protocol.handle(request).await.unwrap();
/// # };
/// ```
///
/// # Range Requests
///
/// Supports HTTP Range headers for streaming:
///
/// ```no_run
/// # #[cfg(feature = "protocol")]
/// # async {
/// # use wvb::protocol::{BundleProtocol, Protocol};
/// # use wvb::source::BundleSource;
/// # use std::sync::Arc;
/// # let source = Arc::new(BundleSource::builder().build());
/// # let protocol = BundleProtocol::new(source);
/// let request = http::Request::builder()
///     .uri("bundle://app/video.mp4")
///     .header("Range", "bytes=0-1023")
///     .body(vec![])
///     .unwrap();
///
/// let response = protocol.handle(request).await.unwrap();
/// assert_eq!(response.status(), 206); // Partial Content
/// # };
/// ```
pub struct BundleProtocol {
  source: Arc<BundleSource>,
  uri_resolver: Box<dyn UriResolver + 'static>,
}

impl BundleProtocol {
  /// Creates a new `BundleProtocol` with the default URI resolver.
  ///
  /// # Arguments
  ///
  /// * `source` - Bundle source to serve files from
  ///
  /// # Example
  ///
  /// ```no_run
  /// # #[cfg(feature = "protocol")]
  /// # {
  /// use wvb::protocol::BundleProtocol;
  /// use wvb::source::BundleSource;
  /// use std::sync::Arc;
  ///
  /// let source = BundleSource::builder()
  ///     .builtin_dir("./bundles")
  ///     .build();
  ///
  /// let protocol = BundleProtocol::new(Arc::new(source));
  /// # }
  /// ```
  pub fn new(source: Arc<BundleSource>) -> Self {
    Self {
      source,
      uri_resolver: Box::new(DefaultUriResolver),
    }
  }
}

#[async_trait]
impl super::Protocol for BundleProtocol {
  async fn handle(&self, request: Request<Vec<u8>>) -> crate::Result<super::ProtocolResponse> {
    let name = self
      .uri_resolver
      .resolve_bundle(request.uri())
      .ok_or(crate::Error::BundleNotFound)?;
    let path = self.uri_resolver.resolve_path(request.uri());

    if !(request.method() == Method::GET || request.method() == Method::HEAD) {
      let response = Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Vec::new().into())?;
      return Ok(response);
    }

    let mut resp = Response::builder();
    let descriptor = self.source.load_descriptor(&name).await?;

    if let Some(entry) = descriptor.index().get_entry(&path) {
      let resp_headers = resp.headers_mut().unwrap();
      resp_headers.clone_from(entry.headers());
      resp_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(entry.content_type()).unwrap(),
      );
      resp_headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(entry.content_length()),
      );

      if let Some(range_header) = request
        .headers()
        .get(header::RANGE)
        .and_then(|x| x.to_str().map(|x| x.to_string()).ok())
      {
        resp_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        resp_headers.insert(
          header::ACCESS_CONTROL_EXPOSE_HEADERS,
          HeaderValue::from_static("content-range"),
        );

        let len = entry.content_length();
        let not_stisifiable = || {
          Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new().into())
            .map_err(Into::into)
        };

        let ranges = if let Ok(ranges) = HttpRange::parse(&range_header, len) {
          ranges
            .iter()
            // map the output to spec range <start-end>, example: 0-499
            .map(|x| (x.start, x.start + x.length - 1))
            .collect::<Vec<_>>()
        } else {
          return not_stisifiable();
        };

        /// The Maximum bytes we send in one range
        const MAX_LEN: u64 = 1000 * 1024;
        let adjust_end =
          |start: u64, end: u64, len: u64| start + (end - start).min(len - start).min(MAX_LEN - 1);

        // signle-part range header
        let response = if ranges.len() == 1 {
          let &(start, mut end) = ranges.first().unwrap();
          // check if a range is not satisfiable
          //
          // this should be already taken care of by the range parsing library
          // but checking here again for extra assurance
          if start >= len || end >= len || end < start {
            return not_stisifiable();
          }
          end = adjust_end(start, end, len);

          resp_headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{len}")).unwrap(),
          );
          resp_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(end + 1 - start));
          resp = resp.status(StatusCode::PARTIAL_CONTENT);

          if request.method() == Method::HEAD {
            resp.body(Vec::new().into())
          } else {
            let reader = self.source.reader(&name).await?;
            let buf = if let Some(data) = descriptor.async_get_data(reader, &path).await? {
              extract_buf(&data, start, end)
            } else {
              return not_found();
            };
            resp.body(buf.into())
          }
        } else {
          let ranges = ranges
            .iter()
            .filter_map(|&(start, mut end)| {
              // filter out unsatisfiable ranges
              //
              // this should be already taken care of by the range parsing library
              // but checking here again for extra assurance
              if start >= len || end >= len || end < start {
                None
              } else {
                // adjust end byte for MAX_LEN
                end = adjust_end(start, end, len);
                Some((start, end))
              }
            })
            .collect::<Vec<_>>();

          let boundary = random_boundary();
          let boundary_sep = format!("\r\n--{boundary}\r\n");

          resp_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/byteranges; boundary={boundary}")).unwrap(),
          );
          resp = resp.status(StatusCode::PARTIAL_CONTENT);

          if request.method() == Method::HEAD {
            resp.body(Vec::new().into())
          } else {
            let reader = self.source.reader(&name).await?;
            let buf = if let Some(data) = descriptor.async_get_data(reader, &path).await? {
              let mut buf = Vec::new();
              for (start, end) in ranges {
                buf.write_all(boundary_sep.as_bytes()).await?;
                buf
                  .write_all(
                    format!("{}: {}\r\n", header::CONTENT_TYPE, entry.content_type()).as_bytes(),
                  )
                  .await?;
                buf
                  .write_all(
                    format!("{}: bytes {start}-{end}/{len}\r\n", header::CONTENT_RANGE).as_bytes(),
                  )
                  .await?;
                buf.write_all("\r\n".as_bytes()).await?;

                let range_buf = extract_buf(&data, start, end);
                buf.extend_from_slice(&range_buf);
              }
              buf.write_all(boundary_sep.as_bytes()).await?;
              buf
            } else {
              return not_found();
            };
            resp.body(buf.into())
          }
        }?;
        return Ok(response);
      }

      if request.method() == Method::HEAD {
        let response = resp.body(Vec::new().into())?;
        return Ok(response);
      }

      let reader = self.source.reader(&name).await?;
      let data = if let Some(data) = descriptor.async_get_data(reader, &path).await? {
        data
      } else {
        return not_found();
      };

      let response = resp.body(data.into())?;
      Ok(response)
    } else {
      not_found()
    }
  }
}

fn not_found() -> crate::Result<super::ProtocolResponse> {
  let resp = Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(Vec::new().into())?;
  Ok(resp)
}

fn random_boundary() -> String {
  let mut values = [0_u8; 30];
  getrandom::fill(&mut values).expect("failed to get random bytes");
  values[..]
    .iter()
    .map(|&val| format!("{val:x}"))
    .fold(String::new(), |mut acc, x| {
      acc.push_str(x.as_str());
      acc
    })
}

fn extract_buf(data: &[u8], start: u64, end: u64) -> Vec<u8> {
  let data_len = data.len() as u64;
  let start_i = start.min(data_len);
  let end_i = end.min(data_len.saturating_sub(1));

  let capacity = end + 1 - start;
  let mut buf = Vec::with_capacity(capacity as usize);
  if start_i <= end_i {
    let s = start as usize;
    let e = (end + 1) as usize;
    buf.extend_from_slice(&data[s..e]);
  }
  buf
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::protocol::Protocol;
  use crate::testing::Fixtures;

  #[tokio::test]
  async fn smoke() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/index.html")
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
          .uri("https://app.wvb/not_found.html")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 404);
    let mut handlers = vec![];
    for _ in 1..100 {
      let p = protocol.clone();
      let handle = tokio::spawn(async move {
        p.handle(
          Request::builder()
            .uri("https://app.wvb/index.html")
            .method("GET")
            .body(vec![])
            .unwrap(),
        )
        .await
      });
      handlers.push(handle);
    }
    for h in handlers {
      let resp = h.await.unwrap().unwrap();
      assert_eq!(resp.status(), 200);
    }
  }

  #[tokio::test]
  async fn resolve_index_html() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
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
    assert_eq!(resp.status(), 200);
  }

  #[tokio::test]
  async fn content_type() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/chunks/framework-98177fb2e8834792.js")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_TYPE).unwrap(),
      HeaderValue::from_static("text/javascript")
    );
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/css/fbfc89e8c66c1961.css")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_TYPE).unwrap(),
      HeaderValue::from_static("text/css")
    );
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/media/build.583ad785.png")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_TYPE).unwrap(),
      HeaderValue::from_static("image/png")
    );
  }

  #[tokio::test]
  async fn content_length() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/chunks/framework-98177fb2e8834792.js")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_LENGTH).unwrap(),
      HeaderValue::from_static("139833")
    );
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/css/fbfc89e8c66c1961.css")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_LENGTH).unwrap(),
      HeaderValue::from_static("13926")
    );
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/media/build.583ad785.png")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(
      resp.headers().get(header::CONTENT_LENGTH).unwrap(),
      HeaderValue::from_static("475918")
    );
  }

  #[tokio::test]
  async fn not_found() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/path/does/not/exists")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 404);
  }

  #[tokio::test]
  async fn bundle_not_found() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let err = protocol
      .handle(
        Request::builder()
          .uri("https://not_exsits_bundle.wvb")
          .method("GET")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap_err();
    assert!(matches!(err, crate::Error::BundleNotFound));
  }

  #[tokio::test]
  async fn head_request() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/_next/static/chunks/framework-98177fb2e8834792.js")
          .method("HEAD")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
      resp.headers().get(header::CONTENT_TYPE).unwrap(),
      HeaderValue::from_static("text/javascript")
    );
  }

  #[tokio::test]
  async fn partial_request() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/build.png")
          .method("GET")
          .header(header::RANGE, "bytes=0-100")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 206);
    assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
    assert_eq!(
      resp.headers().get(header::CONTENT_RANGE).unwrap(),
      "bytes 0-100/475918"
    );
    assert_eq!(resp.headers().get(header::CONTENT_LENGTH).unwrap(), "101");
    let body = resp.body().to_vec();
    assert_eq!(
      body,
      vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 5, 192, 0, 0, 2, 244,
        8, 6, 0, 0, 0, 43, 255, 148, 215, 0, 0, 12, 107, 105, 67, 67, 80, 73, 67, 67, 32, 80, 114,
        111, 102, 105, 108, 101, 0, 0, 72, 137, 149, 87, 7, 88, 83, 201, 22, 158, 91, 146, 144,
        144, 208, 2, 8, 72, 9, 189, 35, 82, 3, 72, 9, 161, 5, 144, 94, 4, 27, 33, 9, 36, 148, 24,
        19, 130, 138, 189, 44, 42, 184, 118, 17, 197, 138
      ]
    );
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/build.png")
          .method("GET")
          .header(header::RANGE, "bytes=0-100,200-500")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 206);
    assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
    assert!(
      resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("multipart/byteranges; boundary=")
    );
  }

  #[tokio::test]
  async fn not_allowed() {
    let fixture = Fixtures::bundles();
    let source = Arc::new(
      BundleSource::builder()
        .builtin_dir(fixture.get_path("builtin"))
        .remote_dir(fixture.get_path("remote"))
        .build(),
    );
    let protocol = Arc::new(BundleProtocol::new(source.clone()));
    let resp = protocol
      .handle(
        Request::builder()
          .uri("https://app.wvb/build.png")
          .method("POST")
          .body(vec![])
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), 405);
  }
}
