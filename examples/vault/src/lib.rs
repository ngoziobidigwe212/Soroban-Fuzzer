//! A token vault: users deposit a Stellar asset, can move their deposit to
//! another user inside the vault, and withdraw it again.
//!
//! The crate also ships (behind the `testutils` feature) the
//! [`soroban-fuzzer`](soroban_fuzzer) harness for the vault and five buggy
//! variants of it, one per bug class the fuzzer detects. The tests in
//! `src/test` and the cargo-fuzz targets in `/fuzz` use them.
#![no_std]

#[cfg(any(test, feature = "testutils"))]
extern crate std;

mod contract;

pub use contract::{DataKey, Vault, VaultClient, VaultError, BUMP_THRESHOLD, BUMP_TO};

#[cfg(any(test, feature = "testutils"))]
pub mod buggy;
#[cfg(any(test, feature = "testutils"))]
pub mod harness;

#[cfg(test)]
mod test;
