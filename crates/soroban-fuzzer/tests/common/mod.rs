//! A vault contract, a fuzz harness for it, and buggy variants of the vault.
//!
//! Every variant exposes the same interface, so one generated client
//! (`VaultClient`) and one harness drive all of them.
#![allow(dead_code)]

use std::{collections::BTreeMap, marker::PhantomData};

use soroban_fuzzer::prelude::*;
use soroban_sdk::{contracterror, contracttype, Address, Env};

pub const DAY_IN_LEDGERS: u32 = 17_280;
pub const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
pub const BUMP_TO: u32 = 30 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount = 1,
    InsufficientBalance = 2,
    Overflow = 3,
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Balance(Address),
    Total,
    Deposit(u32),
    DepositCount,
}

/// The correct implementation.
pub mod vault {
    use super::*;
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    pub struct Vault;

    fn bump_instance(env: &Env) {
        env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
    }

    fn read_balance(env: &Env, who: &Address) -> i128 {
        let key = DataKey::Balance(who.clone());
        let s = env.storage().persistent();
        match s.get::<_, i128>(&key) {
            Some(b) => {
                s.extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
                b
            }
            None => 0,
        }
    }

    fn write_balance(env: &Env, who: &Address, amount: i128) {
        let key = DataKey::Balance(who.clone());
        let s = env.storage().persistent();
        s.set(&key, &amount);
        s.extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
    }

    fn total(env: &Env) -> i128 {
        env.storage().instance().get(&DataKey::Total).unwrap_or(0)
    }

    #[contractimpl]
    impl Vault {
        pub fn __constructor(env: Env) {
            env.storage().instance().set(&DataKey::Total, &0i128);
            bump_instance(&env);
        }

        pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), VaultError> {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            bump_instance(&env);
            let bal = read_balance(&env, &from)
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            let total = total(&env)
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            write_balance(&env, &from, bal);
            env.storage().instance().set(&DataKey::Total, &total);
            Ok(())
        }

        pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), VaultError> {
            to.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            bump_instance(&env);
            let bal = read_balance(&env, &to);
            if bal < amount {
                return Err(VaultError::InsufficientBalance);
            }
            write_balance(&env, &to, bal - amount);
            env.storage()
                .instance()
                .set(&DataKey::Total, &(total(&env) - amount));
            Ok(())
        }

        pub fn transfer(
            env: Env,
            from: Address,
            to: Address,
            amount: i128,
        ) -> Result<(), VaultError> {
            from.require_auth();
            if amount <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            bump_instance(&env);
            let from_bal = read_balance(&env, &from);
            if from_bal < amount {
                return Err(VaultError::InsufficientBalance);
            }
            write_balance(&env, &from, from_bal - amount);
            let to_bal = read_balance(&env, &to);
            write_balance(&env, &to, to_bal + amount);
            Ok(())
        }

        pub fn balance(env: Env, who: Address) -> i128 {
            read_balance(&env, &who)
        }

        pub fn total(env: Env) -> i128 {
            total(&env)
        }
    }
}

pub use vault::VaultClient;

/// How a vault variant gets deployed.
pub trait Variant {
    fn register(env: &Env) -> Address;
}

impl Variant for vault::Vault {
    fn register(env: &Env) -> Address {
        env.register(vault::Vault, ())
    }
}

pub const ACTORS: u8 = 4;

#[derive(Clone, Debug)]
pub enum Action {
    Deposit(ActorId, i128),
    Withdraw(ActorId, i128),
    Transfer(ActorId, ActorId, i128),
}

/// Harness: a model of balances checked against any vault variant.
pub struct VaultHarness<V> {
    pub id: Address,
    pub actors: Actors,
    pub balances: BTreeMap<ActorId, i128>,
    _v: PhantomData<V>,
}

impl<V> VaultHarness<V> {
    fn bal(&self, a: ActorId) -> i128 {
        self.balances.get(&a).copied().unwrap_or(0)
    }

    fn total(&self) -> i128 {
        // The model total cannot overflow: the contract caps it at i128::MAX.
        self.balances.values().sum()
    }
}

type TryResult =
    Result<Result<(), soroban_sdk::ConversionError>, Result<VaultError, soroban_sdk::InvokeError>>;

/// Compare a contract result with what the model expects.
///
/// A host-level failure (the contract trapped, e.g. on an overflow panic) is
/// never an expected outcome, so it is re-raised as a panic and reported as
/// `UnexpectedPanic` rather than a model mismatch.
fn compare(
    name: &str,
    got: TryResult,
    expected: Result<(), VaultError>,
) -> Result<Outcome, String> {
    if let Err(Err(e)) = &got {
        panic!("{name}: contract trapped: {e:?}");
    }
    match (&got, expected) {
        (Ok(Ok(())), Ok(())) => Ok(Outcome::Executed),
        (Err(Ok(e)), Err(x)) if *e == x => Ok(Outcome::Rejected),
        _ => Err(format!(
            "{name}: contract returned {got:?}, model expected {expected:?}"
        )),
    }
}

impl<V: Variant> Harness for VaultHarness<V> {
    type Action = Action;

    fn setup(env: &Env) -> Self {
        Self {
            id: V::register(env),
            actors: Actors::generate(env, ACTORS as usize),
            balances: BTreeMap::new(),
            _v: PhantomData,
        }
    }

    fn actions() -> BoxedStrategy<Action> {
        prop_oneof![
            (actor(ACTORS), amount()).prop_map(|(a, x)| Action::Deposit(a, x)),
            (actor(ACTORS), amount()).prop_map(|(a, x)| Action::Withdraw(a, x)),
            (actor(ACTORS), actor(ACTORS), amount())
                .prop_map(|(a, b, x)| Action::Transfer(a, b, x)),
        ]
        .boxed()
    }

    fn apply(&mut self, env: &Env, action: &Action) -> Result<Outcome, String> {
        let client = VaultClient::new(env, &self.id);
        match *action {
            Action::Deposit(a, x) => {
                let a = self.actors.id(a);
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.bal(a).checked_add(x).is_none()
                    || self.total().checked_add(x).is_none()
                {
                    Err(VaultError::Overflow)
                } else {
                    Ok(())
                };
                let out = compare(
                    "deposit",
                    client.try_deposit(self.actors.get(a), &x),
                    expected,
                )?;
                if expected.is_ok() {
                    *self.balances.entry(a).or_default() += x;
                }
                Ok(out)
            }
            Action::Withdraw(a, x) => {
                let a = self.actors.id(a);
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.bal(a) < x {
                    Err(VaultError::InsufficientBalance)
                } else {
                    Ok(())
                };
                let out = compare(
                    "withdraw",
                    client.try_withdraw(self.actors.get(a), &x),
                    expected,
                )?;
                if expected.is_ok() {
                    *self.balances.entry(a).or_default() -= x;
                }
                Ok(out)
            }
            Action::Transfer(a, b, x) => {
                let (a, b) = (self.actors.id(a), self.actors.id(b));
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.bal(a) < x {
                    Err(VaultError::InsufficientBalance)
                } else {
                    Ok(())
                };
                let got = client.try_transfer(self.actors.get(a), self.actors.get(b), &x);
                let out = compare("transfer", got, expected)?;
                if expected.is_ok() {
                    *self.balances.entry(a).or_default() -= x;
                    *self.balances.entry(b).or_default() += x;
                }
                Ok(out)
            }
        }
    }

    fn invariants(&self, env: &Env) -> Result<(), String> {
        let client = VaultClient::new(env, &self.id);
        for (id, addr) in self.actors.iter() {
            let (got, want) = (client.balance(addr), self.bal(id));
            if got != want {
                return Err(format!(
                    "balance of actor {}: contract {got}, model {want}",
                    id.0
                ));
            }
        }
        let (got, want) = (client.total(), self.total());
        if got != want {
            return Err(format!("total: contract {got}, model {want}"));
        }
        Ok(())
    }

    fn required_auths(&self, _env: &Env, action: &Action) -> Vec<Address> {
        match *action {
            Action::Deposit(a, _) | Action::Withdraw(a, _) | Action::Transfer(a, _, _) => {
                vec![self.actors.get(a).clone()]
            }
        }
    }
}
