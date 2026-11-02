//! Each module below is the vault with one deliberate Soroban bug. The
//! fuzzer must find every one of them, classify it correctly and shrink it to
//! a short reproduction.

mod common;

use common::{DataKey, Variant, VaultError, VaultHarness, BUMP_THRESHOLD, BUMP_TO};
use soroban_fuzzer::prelude::*;
use soroban_sdk::{contract, contractimpl, Address, Env};

/// Seeds tried per bug, so detection does not depend on one lucky seed.
const SEEDS: [u64; 3] = [1, 42, 0xdead_beef];

/// Shared pieces for variants that only differ in one function.
macro_rules! vault_variant {
    ($name:ident, deposit = $deposit:expr, withdraw = $withdraw:expr, balance = $balance:expr) => {
        #[contract]
        pub struct $name;

        #[contractimpl]
        impl $name {
            pub fn __constructor(env: Env) {
                env.storage().instance().set(&DataKey::Total, &0i128);
                env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
            }
            pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), VaultError> {
                env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
                ($deposit)(&env, from, amount)
            }
            pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), VaultError> {
                env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
                ($withdraw)(&env, to, amount)
            }
            pub fn transfer(
                _env: Env,
                _from: Address,
                _to: Address,
                _amount: i128,
            ) -> Result<(), VaultError> {
                Err(VaultError::InvalidAmount)
            }
            pub fn balance(env: Env, who: Address) -> i128 {
                ($balance)(&env, &who)
            }
            pub fn total(env: Env) -> i128 {
                env.storage().instance().get(&DataKey::Total).unwrap_or(0)
            }
        }

        impl Variant for $name {
            fn register(env: &Env) -> Address {
                env.register($name, ())
            }
        }
    };
}

// Simple persistent-storage helpers used by most variants.
fn get_p(env: &Env, who: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(who.clone()))
        .unwrap_or(0)
}
fn set_p(env: &Env, who: &Address, v: i128) {
    let key = DataKey::Balance(who.clone());
    env.storage().persistent().set(&key, &v);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
}
fn add_total(env: &Env, delta: i128) -> Result<(), VaultError> {
    let t: i128 = env.storage().instance().get(&DataKey::Total).unwrap_or(0);
    let t = t.checked_add(delta).ok_or(VaultError::Overflow)?;
    env.storage().instance().set(&DataKey::Total, &t);
    Ok(())
}
fn checked_deposit(env: &Env, from: Address, amount: i128) -> Result<(), VaultError> {
    from.require_auth();
    if amount <= 0 {
        return Err(VaultError::InvalidAmount);
    }
    let b = get_p(env, &from)
        .checked_add(amount)
        .ok_or(VaultError::Overflow)?;
    add_total(env, amount)?;
    set_p(env, &from, b);
    Ok(())
}
fn checked_withdraw(env: &Env, to: Address, amount: i128) -> Result<(), VaultError> {
    to.require_auth();
    if amount <= 0 {
        return Err(VaultError::InvalidAmount);
    }
    let b = get_p(env, &to);
    if b < amount {
        return Err(VaultError::InsufficientBalance);
    }
    set_p(env, &to, b - amount);
    add_total(env, -amount)
}

/// The harness expects `transfer` to work, but these variants stub it out.
/// Restrict the generated actions to deposits and withdrawals.
struct NoTransfer<V>(VaultHarness<V>);

impl<V: Variant> Harness for NoTransfer<V> {
    type Action = common::Action;
    fn setup(env: &Env) -> Self {
        Self(VaultHarness::setup(env))
    }
    fn actions() -> BoxedStrategy<common::Action> {
        use common::{Action, ACTORS};
        prop_oneof![
            (actor(ACTORS), amount()).prop_map(|(a, x)| Action::Deposit(a, x)),
            (actor(ACTORS), amount()).prop_map(|(a, x)| Action::Withdraw(a, x)),
        ]
        .boxed()
    }
    fn apply(&mut self, env: &Env, a: &common::Action) -> Result<Outcome, String> {
        self.0.apply(env, a)
    }
    fn invariants(&self, env: &Env) -> Result<(), String> {
        self.0.invariants(env)
    }
    fn required_auths(&self, env: &Env, a: &common::Action) -> Vec<Address> {
        self.0.required_auths(env, a)
    }
}

fn expect<V: Variant>(
    kind: FailureKind,
    configure: impl Fn(Fuzzer<NoTransfer<V>>) -> Fuzzer<NoTransfer<V>>,
) {
    for seed in SEEDS {
        let err = configure(Fuzzer::new().seed(seed).cases(256))
            .run()
            .expect_err("the fuzzer should have found the bug");
        println!("{err}");
        assert_eq!(err.kind, kind, "wrong classification:\n{err}");
        assert!(err.steps.len() <= 6, "case was not shrunk well:\n{err}");
    }
}

// --- Bug 1: withdraw forgets `require_auth` -------------------------------

mod missing_auth {
    use super::*;
    vault_variant!(
        MissingAuthVault,
        deposit = checked_deposit,
        withdraw = |env: &Env, to: Address, amount: i128| {
            // BUG: anyone can withdraw anyone's funds.
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            let b = get_p(env, &to);
            if b < amount {
                return Err(VaultError::InsufficientBalance);
            }
            set_p(env, &to, b - amount);
            add_total(env, -amount)
        },
        balance = get_p
    );
}

#[test]
fn detects_missing_require_auth() {
    expect::<missing_auth::MissingAuthVault>(FailureKind::MissingAuth, |f| f);
}

// --- Bug 2: balances kept in temporary storage ---------------------------

mod temp_storage {
    use super::*;
    fn get_t(env: &Env, who: &Address) -> i128 {
        env.storage()
            .temporary()
            .get(&DataKey::Balance(who.clone()))
            .unwrap_or(0)
    }
    vault_variant!(
        TempStorageVault,
        deposit = |env: &Env, from: Address, amount: i128| {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            let b = get_t(env, &from)
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            add_total(env, amount)?;
            // BUG: temporary entries are deleted for good once their TTL ends.
            env.storage().temporary().set(&DataKey::Balance(from), &b);
            Ok(())
        },
        withdraw = |env: &Env, to: Address, amount: i128| {
            to.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            let b = get_t(env, &to);
            if b < amount {
                return Err(VaultError::InsufficientBalance);
            }
            env.storage()
                .temporary()
                .set(&DataKey::Balance(to), &(b - amount));
            add_total(env, -amount)
        },
        balance = get_t
    );
}

#[test]
fn detects_balances_in_temporary_storage() {
    expect::<temp_storage::TempStorageVault>(FailureKind::InvariantViolated, |f| f);
}

#[test]
fn temporary_storage_bug_needs_ledger_jumps() {
    // Without time passing, the temporary-storage vault looks correct.
    Fuzzer::<NoTransfer<temp_storage::TempStorageVault>>::new()
        .seed(1)
        .cases(64)
        .without_ledger_jumps()
        .run()
        .unwrap();
}

// --- Bug 3: persistent balances never have their TTL extended ------------

mod no_ttl {
    use super::*;
    fn set_no_bump(env: &Env, who: &Address, v: i128) {
        // BUG: no extend_ttl, so the entry archives after the minimum TTL.
        env.storage()
            .persistent()
            .set(&DataKey::Balance(who.clone()), &v);
    }
    vault_variant!(
        NoTtlVault,
        deposit = |env: &Env, from: Address, amount: i128| {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            let b = get_p(env, &from)
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            add_total(env, amount)?;
            set_no_bump(env, &from, b);
            Ok(())
        },
        withdraw = checked_withdraw,
        balance = get_p
    );
}

#[test]
fn detects_missing_ttl_extension() {
    expect::<no_ttl::NoTtlVault>(FailureKind::ArchivedEntry, |f| f);
}

// --- Bug 4: unchecked arithmetic -----------------------------------------

mod overflow {
    use super::*;
    vault_variant!(
        OverflowVault,
        deposit = |env: &Env, from: Address, amount: i128| {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            // BUG: plain `+` panics (or wraps) on overflow.
            let b = get_p(env, &from) + amount;
            let t: i128 = env.storage().instance().get(&DataKey::Total).unwrap_or(0);
            env.storage().instance().set(&DataKey::Total, &(t + amount));
            set_p(env, &from, b);
            Ok(())
        },
        withdraw = checked_withdraw,
        balance = get_p
    );
}

#[test]
fn detects_arithmetic_overflow() {
    expect::<overflow::OverflowVault>(FailureKind::UnexpectedPanic, |f| f);
}

// --- Bug 5: storage reads grow with every deposit ------------------------

mod unbounded {
    use super::*;
    fn sum_deposits(env: &Env, who: &Address) -> i128 {
        let n: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DepositCount)
            .unwrap_or(0);
        let mut sum = 0i128;
        // BUG: one storage read per deposit ever made, by anyone.
        for i in 0..n {
            if let Some((owner, amount)) = env
                .storage()
                .persistent()
                .get::<_, (Address, i128)>(&DataKey::Deposit(i))
            {
                if owner == *who {
                    sum = sum.wrapping_add(amount);
                }
            }
        }
        sum.wrapping_sub(get_p(env, who)) // `Balance` holds the amount withdrawn
    }
    vault_variant!(
        UnboundedVault,
        deposit = |env: &Env, from: Address, amount: i128| {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            sum_deposits(env, &from)
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            add_total(env, amount)?;
            let s = env.storage().persistent();
            let n: u32 = s.get(&DataKey::DepositCount).unwrap_or(0);
            s.set(&DataKey::Deposit(n), &(from, amount));
            s.set(&DataKey::DepositCount, &(n + 1));
            s.extend_ttl(&DataKey::Deposit(n), BUMP_THRESHOLD, BUMP_TO);
            s.extend_ttl(&DataKey::DepositCount, BUMP_THRESHOLD, BUMP_TO);
            Ok(())
        },
        withdraw = |env: &Env, to: Address, amount: i128| {
            to.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            if sum_deposits(env, &to) < amount {
                return Err(VaultError::InsufficientBalance);
            }
            set_p(env, &to, get_p(env, &to).wrapping_add(amount));
            add_total(env, -amount)
        },
        balance = sum_deposits
    );
}

#[test]
fn detects_unbounded_storage_reads() {
    // A team budget of 8 footprint entries per call: the vault reads one
    // entry per historical deposit, so it blows through it as usage grows.
    expect::<unbounded::UnboundedVault>(FailureKind::ResourceBudgetExceeded, |f| {
        f.steps(1..=48).resource_budget(ResourceBudget {
            max_footprint_entries: 8,
            ..ResourceBudget::default()
        })
    });
}

#[test]
fn unbounded_vault_hits_network_limits() {
    // With only the network limits (no headroom), enough deposits make the
    // call itself fail inside the Env.
    let fuzzer = Fuzzer::<NoTransfer<unbounded::UnboundedVault>>::new()
        .resource_budget(ResourceBudget::UNLIMITED)
        .without_ledger_jumps();
    let steps = (0..420)
        .map(|_| Step::Call(common::Action::Deposit(ActorId(0), 1)))
        .collect();
    let err = fuzzer.replay(steps).unwrap_err();
    assert_eq!(err.kind, FailureKind::ResourceLimitExceeded, "{err}");
}
