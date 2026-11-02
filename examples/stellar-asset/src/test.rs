use soroban_fuzzer::prelude::*;

use crate::harness::{Action, Expiry, SacHarness};

/// The real SAC must pass every check, including the unsigned-call probe,
/// with no false alarms. A failure here means either a SAC bug or a gap in
/// the harness's model of it.
#[test]
fn stellar_asset_contract_passes_every_check() {
    for seed in [1, 2, 3] {
        let report = Fuzzer::<SacHarness>::new()
            .seed(seed)
            .cases(128)
            .check_unsigned_calls()
            .check();
        assert!(report.executed > report.rejected / 4, "{report}");
    }
}

/// An allowance can be spent up to and including its `live_until` ledger,
/// and reads as 0 after it.
#[test]
fn allowance_expires_after_live_until() {
    let (a, b) = (ActorId(0), ActorId(1));
    let steps = vec![
        Step::Call(Action::Approve(a, b, Amount::Exact(50), Expiry::In(10))),
        Step::AdvanceLedger(10),
        Step::Call(Action::TransferFrom(b, a, b, Amount::Exact(20))),
        Step::AdvanceLedger(1),
        // Expired now: the model expects an Allowance error.
        Step::Call(Action::TransferFrom(b, a, b, Amount::Exact(1))),
    ];
    Fuzzer::<SacHarness>::new().replay(steps).unwrap();
}

/// A deauthorized holder can neither send nor receive, but the admin can
/// still claw its tokens back.
#[test]
fn deauthorized_balance_is_frozen_but_clawbackable() {
    let (a, b) = (ActorId(0), ActorId(1));
    let steps = vec![
        Step::Call(Action::SetAuthorized(a, false)),
        Step::Call(Action::Transfer(a, b, Amount::Exact(1))),
        Step::Call(Action::Transfer(b, a, Amount::Exact(1))),
        Step::Call(Action::Clawback(a, Amount::All)),
        Step::Call(Action::SetAuthorized(a, true)),
        Step::Call(Action::Transfer(b, a, Amount::Exact(1))),
    ];
    Fuzzer::<SacHarness>::new().replay(steps).unwrap();
}
