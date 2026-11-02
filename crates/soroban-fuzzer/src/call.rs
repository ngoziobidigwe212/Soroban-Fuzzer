//! Per-call bookkeeping: lets the runner check *every* contract call an
//! action makes, not only the last one.

use std::{cell::RefCell, fmt::Debug};

use soroban_sdk::{Address, Env, InvokeError};

use crate::{Outcome, ResourceUsage};

/// What was observed for the calls made during one step.
#[derive(Default)]
pub(crate) struct StepRecord {
    pub usages: Vec<ResourceUsage>,
    pub signers: Vec<Address>,
}

thread_local! {
    static RECORDER: RefCell<Option<StepRecord>> = const { RefCell::new(None) };
}

pub(crate) fn start_recording() {
    RECORDER.with(|r| *r.borrow_mut() = Some(StepRecord::default()));
}

pub(crate) fn finish_recording() -> StepRecord {
    RECORDER.with(|r| r.borrow_mut().take().unwrap_or_default())
}

pub(crate) fn pause_recording() -> Option<StepRecord> {
    RECORDER.with(|r| r.borrow_mut().take())
}

pub(crate) fn resume_recording(saved: Option<StepRecord>) {
    RECORDER.with(|r| *r.borrow_mut() = saved);
}

/// Record the resources and signers of the contract call that just finished.
///
/// The runner always records the *last* call of each step by itself. Call
/// this after earlier calls when an action makes several, so that each one
/// is checked against the resource budget and counted for auth checks.
/// [`check_call`] does it for you. Outside a fuzz run this does nothing.
pub fn record_call(env: &Env) {
    RECORDER.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if let Some(u) = ResourceUsage::last(env) {
                rec.usages.push(u);
            }
            rec.signers.extend(env.auths().into_iter().map(|(a, _)| a));
        }
    });
}

/// Compare the result of a `try_` client call with the outcome your model
/// expects, and record the call.
///
/// - `Ok(..)` when `expected` is `Ok(())` → [`Outcome::Executed`]
/// - the expected contract error → [`Outcome::Rejected`]
/// - anything else → `Err(message)`, reported as a model mismatch
/// - a host-level failure (the contract trapped: overflow, `unwrap`, failed
///   auth, …) is never expected, so it panics and is reported as
///   `UnexpectedPanic`.
///
/// ```ignore
/// let got = client.try_withdraw(&user, &amount);
/// check_call(env, "withdraw", got, if enough { Ok(()) } else { Err(Error::Insufficient) })
/// ```
pub fn check_call<T, C, E>(
    env: &Env,
    name: &str,
    got: Result<Result<T, C>, Result<E, InvokeError>>,
    expected: Result<(), E>,
) -> Result<Outcome, String>
where
    T: Debug,
    C: Debug,
    E: Debug + PartialEq,
{
    record_call(env);
    match (got, expected) {
        (Err(Err(e)), _) => panic!("{name}: contract trapped: {e:?}"),
        (Ok(Ok(_)), Ok(())) => Ok(Outcome::Executed),
        (Err(Ok(e)), Err(x)) if e == x => Ok(Outcome::Rejected),
        (got, expected) => Err(format!(
            "{name}: contract returned {got:?}, model expected {expected:?}"
        )),
    }
}
