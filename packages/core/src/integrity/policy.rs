/// How a bundle's integrity metadata is treated when the integrity check runs.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum IntegrityPolicy {
  /// Integrity metadata is required: a bundle without it fails the check.
  Strict,
  /// Integrity metadata is checked when present and tolerated when missing.
  #[default]
  Optional,
  /// The integrity check is disabled.
  Off,
}
