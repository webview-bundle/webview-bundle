use crate::testing::bundle::TestingBundle;
use std::collections::HashSet;
use std::collections::hash_set;

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

  pub fn len(&self) -> usize {
    self.bundles.len()
  }

  pub fn is_empty(&self) -> bool {
    self.bundles.is_empty()
  }

  pub fn iter(&self) -> hash_set::Iter<'_, TestingBundle> {
    self.bundles.iter()
  }
}

impl IntoIterator for TestingBundleCollection {
  type Item = TestingBundle;
  type IntoIter = hash_set::IntoIter<TestingBundle>;

  fn into_iter(self) -> Self::IntoIter {
    self.bundles.into_iter()
  }
}

impl<'a> IntoIterator for &'a TestingBundleCollection {
  type Item = &'a TestingBundle;
  type IntoIter = hash_set::Iter<'a, TestingBundle>;

  fn into_iter(self) -> Self::IntoIter {
    self.bundles.iter()
  }
}

impl FromIterator<TestingBundle> for TestingBundleCollection {
  fn from_iter<T: IntoIterator<Item = TestingBundle>>(iter: T) -> Self {
    Self {
      bundles: iter.into_iter().collect(),
    }
  }
}

impl Extend<TestingBundle> for TestingBundleCollection {
  fn extend<T: IntoIterator<Item = TestingBundle>>(&mut self, iter: T) {
    self.bundles.extend(iter);
  }
}
