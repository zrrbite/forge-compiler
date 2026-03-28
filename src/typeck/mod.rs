pub mod check;
pub mod scope;
pub mod types;

#[cfg(test)]
mod tests;

pub use check::{TypeChecker, TypeError};
pub use types::Ty;
