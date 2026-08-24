use std::collections::HashMap;

/// Parses a [RFC 8941] dictionary whose member values are all strings.
///
/// Member parameters are parsed and dropped. Members with a non-string value, and any input
/// that does not follow the grammar, are rejected with `None`.
///
/// [RFC 8941]: https://www.rfc-editor.org/rfc/rfc8941#name-dictionaries
pub(crate) fn parse_string_dict(input: &str) -> Option<HashMap<String, String>> {
  let bytes = input.as_bytes();
  let mut pos = 0;
  let mut dict = HashMap::new();
  skip_ows(bytes, &mut pos);
  while pos < bytes.len() {
    let key = take_key(bytes, &mut pos)?;
    if bytes.get(pos) != Some(&b'=') {
      return None;
    }
    pos += 1;
    let value = take_string(bytes, &mut pos)?;
    skip_params(bytes, &mut pos)?;
    dict.insert(key, value);
    skip_ows(bytes, &mut pos);
    match bytes.get(pos) {
      Some(b',') => {
        pos += 1;
        skip_ows(bytes, &mut pos);
        if pos >= bytes.len() {
          return None;
        }
      }
      Some(_) => return None,
      None => break,
    }
  }
  Some(dict)
}

fn skip_ows(bytes: &[u8], pos: &mut usize) {
  while matches!(bytes.get(*pos), Some(b' ' | b'\t')) {
    *pos += 1;
  }
}

fn take_key(bytes: &[u8], pos: &mut usize) -> Option<String> {
  let start = *pos;
  match bytes.get(*pos) {
    Some(c) if c.is_ascii_lowercase() || *c == b'*' => *pos += 1,
    _ => return None,
  }
  while let Some(c) = bytes.get(*pos) {
    if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-' | b'.' | b'*') {
      *pos += 1;
    } else {
      break;
    }
  }
  Some(str::from_utf8(&bytes[start..*pos]).ok()?.to_owned())
}

fn take_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
  if bytes.get(*pos) != Some(&b'"') {
    return None;
  }
  *pos += 1;
  let mut out = String::new();
  loop {
    let c = *bytes.get(*pos)?;
    *pos += 1;
    match c {
      b'"' => return Some(out),
      b'\\' => {
        let escaped = *bytes.get(*pos)?;
        *pos += 1;
        if escaped != b'"' && escaped != b'\\' {
          return None;
        }
        out.push(escaped as char);
      }
      0x20..=0x7e => out.push(c as char),
      _ => return None,
    }
  }
}

fn skip_params(bytes: &[u8], pos: &mut usize) -> Option<()> {
  while bytes.get(*pos) == Some(&b';') {
    *pos += 1;
    while bytes.get(*pos) == Some(&b' ') {
      *pos += 1;
    }
    take_key(bytes, pos)?;
    if bytes.get(*pos) == Some(&b'=') {
      *pos += 1;
      if bytes.get(*pos) == Some(&b'"') {
        take_string(bytes, pos)?;
      } else {
        let start = *pos;
        while let Some(c) = bytes.get(*pos) {
          if matches!(c, b';' | b',' | b' ' | b'\t') {
            break;
          }
          *pos += 1;
        }
        if start == *pos {
          return None;
        }
      }
    }
  }
  Some(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn dict(input: &str) -> Option<Vec<(String, String)>> {
    parse_string_dict(input).map(|x| {
      let mut members = x.into_iter().collect::<Vec<_>>();
      members.sort();
      members
    })
  }

  fn member(key: &str, value: &str) -> (String, String) {
    (key.to_owned(), value.to_owned())
  }

  #[test]
  fn parses_members() {
    assert_eq!(
      dict(r#"key_id="somekey", alg="alg", sig="value""#),
      Some(vec![
        member("alg", "alg"),
        member("key_id", "somekey"),
        member("sig", "value"),
      ])
    );
  }

  #[test]
  fn parses_without_optional_whitespace() {
    assert_eq!(
      dict(r#"alg="alg",sig="value""#),
      Some(vec![member("alg", "alg"), member("sig", "value")])
    );
    assert_eq!(
      dict("\t alg=\"alg\" \t, \tsig=\"value\" "),
      Some(vec![member("alg", "alg"), member("sig", "value")])
    );
  }

  #[test]
  fn parses_escapes() {
    assert_eq!(
      dict(r#"sig="a\"b\\c""#),
      Some(vec![member("sig", "a\"b\\c")])
    );
  }

  #[test]
  fn keeps_last_duplicated_member() {
    assert_eq!(
      dict(r#"sig="first", sig="last""#),
      Some(vec![member("sig", "last")])
    );
  }

  #[test]
  fn drops_parameters() {
    assert_eq!(
      dict(r#"sig="value";expires=123;flag, alg="alg";name="v,alue""#),
      Some(vec![member("alg", "alg"), member("sig", "value")])
    );
  }

  #[test]
  fn parses_empty_dict() {
    assert_eq!(dict(""), Some(vec![]));
    assert_eq!(dict("  "), Some(vec![]));
  }

  #[test]
  fn rejects_malformed_dict() {
    assert_eq!(dict(r#"sig="value"#), None);
    assert_eq!(dict(r#"sig=value"#), None);
    assert_eq!(dict("sig"), None);
    assert_eq!(dict(r#"SIG="value""#), None);
    assert_eq!(dict(r#"sig="value","#), None);
    assert_eq!(dict(r#"sig="value" alg="alg""#), None);
    assert_eq!(dict(r#"sig="va\lue""#), None);
    assert_eq!(dict("sig=\"va\nlue\""), None);
  }
}
