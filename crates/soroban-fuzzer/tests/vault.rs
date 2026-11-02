//! The correct vault must survive fuzzing with every check enabled.

mod common;

use common::{vault::Vault, Action, VaultHarness};
use soroban_fuzzer::prelude::*;

#[test]
fn correct_vault_passes() {
    let report = Fuzzer::<VaultHarness<Vault>>::new().cases(128).check();
    assert!(report.executed > 0, "no call was ever executed: {report}");
    assert!(
        report.ledger_jumps > 0,
        "no ledger jump was generated: {report}"
    );
    assert!(
        report.peak.instructions > 0,
        "resources were not metered: {report}"
    );
}

#[test]
fn fixed_seed_is_deterministic() {
    let a = Fuzzer::<VaultHarness<Vault>>::new()
        .cases(16)
        .seed(7)
        .run()
        .unwrap();
    let b = Fuzzer::<VaultHarness<Vault>>::new()
        .cases(16)
        .seed(7)
        .run()
        .unwrap();
    assert_eq!(
        (a.steps, a.executed, a.rejected, a.ledger_jumps, a.peak),
        (b.steps, b.executed, b.rejected, b.ledger_jumps, b.peak)
    );
}

#[test]
fn replay_runs_an_explicit_sequence() {
    let fuzzer = Fuzzer::<VaultHarness<Vault>>::new();
    let steps = vec![
        Step::Call(Action::Deposit(ActorId(0), 100)),
        Step::AdvanceLedger(100_000),
        Step::Call(Action::Transfer(ActorId(0), ActorId(1), 40)),
        Step::Call(Action::Withdraw(ActorId(1), 40)),
        Step::Call(Action::Deposit(ActorId(2), i128::MAX)),
        Step::Call(Action::Deposit(ActorId(3), 1)), // total would overflow: rejected
    ];
    fuzzer.replay(steps).unwrap();
}

#[test]
fn tight_budget_is_reported() {
    let err = Fuzzer::<VaultHarness<Vault>>::new()
        .cases(32)
        .resource_budget(ResourceBudget {
            max_instructions: 1_000,
            ..ResourceBudget::UNLIMITED
        })
        .run()
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::ResourceBudgetExceeded, "{err}");
    assert_eq!(err.steps.len(), 1, "should shrink to a single call: {err}");
}
