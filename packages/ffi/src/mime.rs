// Adapted from https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/mime_type.rs

use std::fmt;

const MIMETYPE_PLAIN: &str = "text/plain";

pub enum MimeType {
  Css,
  Csv,
  Html,
  Ico,
  Js,
  Json,
  Jsonld,
  Mp4,
  OctetStream,
  Rtf,
  Svg,
  Txt,
}

impl fmt::Display for MimeType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mime = match self {
      MimeType::Css => "text/css",
      MimeType::Csv => "text/csv",
      MimeType::Html => "text/html",
      MimeType::Ico => "image/vnd.microsoft.icon",
      MimeType::Js => "text/javascript",
      MimeType::Json => "application/json",
      MimeType::Jsonld => "application/ld+json",
      MimeType::Mp4 => "video/mp4",
      MimeType::OctetStream => "application/octet-stream",
      MimeType::Rtf => "application/rtf",
      MimeType::Svg => "image/svg+xml",
      MimeType::Txt => MIMETYPE_PLAIN,
    };
    write!(f, "{mime}")
  }
}

impl MimeType {
  pub fn parse_from_uri_with_fallback(uri: &str, fallback: MimeType) -> MimeType {
    let suffix = uri.split('.').next_back();
    match suffix {
      Some("bin") => Self::OctetStream,
      Some("css" | "less" | "sass" | "styl") => Self::Css,
      Some("csv") => Self::Csv,
      Some("html") => Self::Html,
      Some("ico") => Self::Ico,
      Some("js" | "mjs") => Self::Js,
      Some("json") => Self::Json,
      Some("jsonld") => Self::Jsonld,
      Some("mp4") => Self::Mp4,
      Some("rtf") => Self::Rtf,
      Some("svg") => Self::Svg,
      Some("txt") => Self::Txt,
      Some(_) => fallback,
      None => Self::OctetStream,
    }
  }

  pub fn parse_with_fallback(content: &[u8], uri: &str, fallback: MimeType) -> String {
    let mime = if uri.ends_with(".svg") {
      None
    } else {
      infer::get(content).map(|info| info.mime_type())
    };
    match mime {
      Some(mime) if mime == MIMETYPE_PLAIN => {
        Self::parse_from_uri_with_fallback(uri, fallback).to_string()
      }
      None => Self::parse_from_uri_with_fallback(uri, fallback).to_string(),
      Some(mime) => mime.to_string(),
    }
  }
}
