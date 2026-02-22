#![cfg(test)]
#![allow(dead_code, unused_imports, unused_variables)]

mod fixtures;
#[cfg(all(feature = "source", feature = "remote"))]
mod mock;
mod temp;

pub use fixtures::*;
#[cfg(all(feature = "source", feature = "remote"))]
pub use mock::*;
pub use temp::*;
