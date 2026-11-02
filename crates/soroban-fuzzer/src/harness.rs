use std::fmt::Debug;

use proptest::strategy::BoxedStrategy;
use soroban_sdk::{Address, Env};

/// What happened when the harness applied an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The contract call went through and changed (or read) state.
    Executed,
    /// The contract rejected the call with an *expected* error, e.g. a
    /// `try_withdraw` that returned `Err(InsufficientBalance)`. Rejected calls
    /// are not auth-checked.
    Rejected,
    /// The harness decided not to call the contract for this action (for
    /// example because a precondition did not hold).
    Skipped,
}

impl Outcome {
    /// Run `update` (typically a model update) only if the call executed,
    /// then return the outcome:
    /// `Ok(check_call(env, "burn", got, expected)?.if_executed(|| model.debit(a, x)))`.
    pub fn if_executed(self, update: impl FnOnce()) -> Self {
        if self == Outcome::Executed {
            update();
        }
        self
    }
}

/// A fuzzing harness for one contract (or a small system of contracts).
///
/// The harness value itself is the "world" for one test case: it typically
/// holds the contract address, a pool of [`Actors`](crate::Actors) and a
/// simple model of the state the contract is expected to have. A fresh
/// harness is built with [`Harness::setup`] for every generated case.
pub trait Harness: Sized {
    /// One user-level operation, e.g. `Deposit(actor, amount)`.
    type Action: Clone + Debug + 'static;

    /// Register contracts, create actors and initial state in a fresh `Env`.
    fn setup(env: &Env) -> Self;

    /// The proptest strategy used to generate actions.
    fn actions() -> BoxedStrategy<Self::Action>;

    /// Apply an action against the contract.
    ///
    /// Return `Err(message)` when the contract behaved differently from what
    /// the model expects (wrong balance, unexpected error code, …). A panic
    /// inside this method is also reported as a failure, so calling the
    /// non-`try_` client functions is fine when an error is never expected.
    fn apply(&mut self, env: &Env, action: &Self::Action) -> Result<Outcome, String>;

    /// Properties that must hold after every step (including ledger jumps).
    fn invariants(&self, _env: &Env) -> Result<(), String> {
        Ok(())
    }

    /// Addresses that *must* have authorised the call made for `action`.
    ///
    /// Evaluated before the action is applied. After an
    /// [`Outcome::Executed`] step the fuzzer checks that each address appears
    /// in `env.auths()` for the last top-level contract call made by `apply`.
    /// This is how missing `require_auth` calls are caught, so make the
    /// state-changing call the last one in `apply`.
    fn required_auths(&self, _env: &Env, _action: &Self::Action) -> Vec<Address> {
        Vec::new()
    }

    /// Contracts whose persistent and instance entries must never expire
    /// within the configured [`LedgerJumps::horizon`](crate::LedgerJumps).
    ///
    /// `None` (the default) watches every contract in the `Env`. Return your
    /// own contracts here if the harness also deploys helpers, such as token
    /// contracts, whose TTLs you do not manage.
    fn watched_contracts(&self) -> Option<Vec<Address>> {
        None
    }
}
