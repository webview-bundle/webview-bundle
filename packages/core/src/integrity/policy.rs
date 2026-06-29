#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum IntegrityPolicy {
  Strict,
  #[default]
  Optional,
  None,
}
