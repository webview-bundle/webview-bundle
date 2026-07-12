use std::fs;
use std::path::PathBuf;

/// The build-script view of the table: just the wire codes, in declaration order. The variants and
/// their attributes are `src/error.rs`'s business.
macro_rules! error_codes {
  ($($(#[$attr:meta])* $variant:ident => $value:literal),+ $(,)?) => {
    const ERROR_CODES: &[&str] = &[$($value,)+];
  };
}

include!("src/error_codes.rs");

fn render_ts() -> String {
  let mut out = String::from(
    "// @generated from `src/error_codes.rs` by `build.rs` — do not edit.\n\n\
     /** The stable code every {@link WebviewBundleError} carries. */\n\
     export type ErrorCode =\n",
  );
  for (i, code) in ERROR_CODES.iter().enumerate() {
    let end = if i == ERROR_CODES.len() - 1 { ";" } else { "" };
    out.push_str(&format!("  | '{code}'{end}\n"));
  }
  out
}

fn main() {
  println!("cargo::rerun-if-changed=src/error_codes.rs");
  println!("cargo::rerun-if-changed=build.rs");

  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib/error-codes.ts");
  let rendered = render_ts();
  // Only write on a change: a build script rewriting a source file on every build churns its mtime,
  // which retriggers watchers (`deno task test --watch`, editors) for no reason.
  if fs::read_to_string(&path).ok().as_deref() != Some(rendered.as_str()) {
    fs::write(&path, &rendered)
      .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
  }
}
