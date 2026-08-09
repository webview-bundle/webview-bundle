use crate::testing::bundle::TestingBundle;
use std::collections::HashSet;

#[derive(Default, Debug, Clone)]
pub struct TestingBundleCollection {
  bundles: HashSet<TestingBundle>,
}

impl TestingBundleCollection {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn get(
    &self,
    bundle_name: impl Into<String>,
    version: impl Into<String>,
  ) -> Option<&TestingBundle> {
    let bundle: TestingBundle = (bundle_name, version).into();
    self.bundles.get(&bundle)
  }

  pub fn insert(&mut self, bundle: TestingBundle) -> bool {
    self.bundles.insert(bundle)
  }

  pub fn remove(&mut self, bundle: TestingBundle) -> bool {
    self.bundles.remove(&bundle)
  }

  pub fn remove_all_for(&mut self, bundle_name: impl Into<String>) -> &mut Self {
    let bundle_name = bundle_name.into();
    self.bundles.retain(|x| x.name() != bundle_name);
    self
  }

  pub fn clear(&mut self) -> &mut Self {
    self.bundles.clear();
    self
  }
}
