//! Structural merge via tree-sitter grammars.
#![warn(missing_docs)]

pub mod entries;
pub mod imports;
pub mod keyed3;
pub mod lang;
pub mod parse;
pub mod propose;
pub mod render;

pub use lang::Lang;
pub use propose::{propose, propose_until, Outcome, Proposal, ProposeError};

/// This crate's version, from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
