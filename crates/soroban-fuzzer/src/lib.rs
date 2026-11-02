//! # soroban-fuzzer
//!
//! Stateful, property-based fuzzing for Soroban smart contracts, running
//! inside the native `soroban-sdk` test environment.
//!
//! You describe *what a user can do* to your contract (an `Action` enum and
//! a [`proptest`] strategy for it) and *what must always be true* (your
//! invariants). The fuzzer then generates random sequences of actions,
//! interleaves them with ledger time jumps, and checks after every step that:
//!
//! - no call panicked unexpectedly (overflows, `unwrap`s, host errors),
//! - no persistent or instance entry **expires** while the contract is still
//!   expected to hold it (missing or too short `extend_ttl`),
//! - every caller you declared as a required signer actually authorised the
//!   call (missing `require_auth`),
//! - CPU, memory, footprint entries, bytes and events stay within a
//!   configurable share of the network limits (unbounded loops / reads),
//! - your own invariants hold (e.g. a model of balances matches the contract).
//!
//! When something fails, the action sequence is **shrunk** to a minimal
//! reproduction and printed together with the seed needed to replay it.
//!
//! ```ignore
//! use soroban_fuzzer::prelude::*;
//!
//! struct VaultFuzz { id: Address, actors: Actors, model: BTreeMap<ActorId, i128> }
//!
//! #[derive(Clone, Debug)]
//! enum Action { Deposit(ActorId, i128), Withdraw(ActorId, i128) }
//!
//! impl Harness for VaultFuzz {
//!     type Action = Action;
//!     fn setup(env: &Env) -> Self { /* register contract, generate actors */ }
//!     fn actions() -> BoxedStrategy<Action> { /* proptest strategy */ }
//!     fn apply(&mut self, env: &Env, a: &Action) -> Result<Outcome, String> { /* call + update model */ }
//!     fn invariants(&self, env: &Env) -> Result<(), String> { /* compare model vs contract */ }
//!     fn required_auths(&self, _env: &Env, a: &Action) -> Vec<Address> { /* who must sign */ }
//! }
//!
//! #[test]
//! fn fuzz_vault() {
//!     Fuzzer::<VaultFuzz>::new().cases(200).check();
//! }
//! ```

mod actors;
mod env;
mod failure;
mod harness;
mod panic;
mod report;
mod resources;
mod runner;
pub mod storage;
pub mod strategies;
pub mod ttl;

pub use actors::{ActorId, Actors};
pub use env::{advance_ledgers, LedgerJumps};
pub use failure::{FailureKind, FuzzFailure};
pub use harness::{Harness, Outcome};
pub use panic::catch_quiet;
pub use report::Report;
pub use resources::{ResourceBudget, ResourceUsage};
pub use runner::{Fuzzer, Step};

/// Re-exported so harnesses don't need a direct dependency to write strategies.
pub use proptest;

/// Everything a typical harness needs in one import.
pub mod prelude {
    pub use crate::strategies::*;
    pub use crate::{
        advance_ledgers, ActorId, Actors, FailureKind, FuzzFailure, Fuzzer, Harness, LedgerJumps,
        Outcome, Report, ResourceBudget, ResourceUsage, Step,
    };
    pub use proptest::prelude::{any, prop_oneof, BoxedStrategy, Just, Strategy};
    pub use soroban_sdk::{Address, Env};
}
