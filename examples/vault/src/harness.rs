//! The soroban-fuzzer harness for the vault.
//!
//! It keeps a model of every actor's vault balance and token wallet, and
//! checks the contract (and the real Stellar Asset Contract holding the
//! funds) against it after every step. The harness is generic over
//! [`Variant`], so the same model drives the correct vault, its compiled
//! Wasm, and each buggy variant.

use std::{collections::BTreeMap, format, marker::PhantomData, string::String, vec, vec::Vec};

use soroban_fuzzer::prelude::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
};

use crate::{Vault, VaultClient, VaultError};

/// Number of users in every case.
pub const ACTORS: u8 = 4;
/// Tokens minted to each user. All wallets together hold `i128::MAX`, so no
/// token balance can overflow, but a single user can still move amounts
/// large enough to overflow careless arithmetic.
pub const WALLET: i128 = i128::MAX / ACTORS as i128;

/// A deployable version of the vault.
pub trait Variant {
    /// Whether the variant implements `transfer` (the buggy ones do not).
    const HAS_TRANSFER: bool = true;
    fn register(env: &Env, token: &Address) -> Address;
}

impl Variant for Vault {
    fn register(env: &Env, token: &Address) -> Address {
        env.register(Vault, (token.clone(),))
    }
}

/// The vault compiled to Wasm (`fixtures/vault.wasm`), so that VM costs are
/// part of the measured resources.
pub struct VaultWasm;

/// Build it with `stellar contract build --package vault-example`, then copy
/// `target/wasm32v1-none/release/vault_example.wasm` here. The repository
/// keeps an empty placeholder; CI builds the real file before testing.
pub const VAULT_WASM: &[u8] = include_bytes!("../fixtures/vault.wasm");

impl Variant for VaultWasm {
    fn register(env: &Env, token: &Address) -> Address {
        env.register(VAULT_WASM, (token.clone(),))
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Deposit(ActorId, Amount),
    Withdraw(ActorId, Amount),
    Transfer(ActorId, ActorId, Amount),
}

pub struct VaultHarness<V> {
    pub vault: Address,
    pub token: Address,
    pub actors: Actors,
    /// Model: each actor's balance inside the vault.
    pub deposits: BTreeMap<ActorId, i128>,
    /// Model: each actor's token wallet.
    pub wallets: BTreeMap<ActorId, i128>,
    _variant: PhantomData<fn() -> V>,
}

// Manual impl: `derive` would needlessly require `V: Clone`.
impl<V> Clone for VaultHarness<V> {
    fn clone(&self) -> Self {
        Self {
            vault: self.vault.clone(),
            token: self.token.clone(),
            actors: self.actors.clone(),
            deposits: self.deposits.clone(),
            wallets: self.wallets.clone(),
            _variant: PhantomData,
        }
    }
}

impl<V> VaultHarness<V> {
    fn deposit_of(&self, a: ActorId) -> i128 {
        self.deposits.get(&a).copied().unwrap_or(0)
    }

    fn wallet_of(&self, a: ActorId) -> i128 {
        self.wallets.get(&a).copied().unwrap_or(0)
    }

    fn total(&self) -> i128 {
        self.deposits.values().sum()
    }
}

impl<V: Variant> Harness for VaultHarness<V> {
    type Action = Action;

    fn setup(env: &Env) -> Self {
        let admin = Address::generate(env);
        let token = env.register_stellar_asset_contract_v2(admin).address();
        let actors = Actors::generate(env, ACTORS as usize);
        let minter = StellarAssetClient::new(env, &token);
        let mut wallets = BTreeMap::new();
        for (id, addr) in actors.iter() {
            minter.mint(addr, &WALLET);
            wallets.insert(id, WALLET);
        }
        Self {
            vault: V::register(env, &token),
            token,
            actors,
            deposits: BTreeMap::new(),
            wallets,
            _variant: PhantomData,
        }
    }

    fn actions() -> BoxedStrategy<Action> {
        let deposit = (actor(ACTORS), relative_amount()).prop_map(|(a, x)| Action::Deposit(a, x));
        let withdraw = (actor(ACTORS), relative_amount()).prop_map(|(a, x)| Action::Withdraw(a, x));
        if V::HAS_TRANSFER {
            let transfer = (actor(ACTORS), actor(ACTORS), relative_amount())
                .prop_map(|(a, b, x)| Action::Transfer(a, b, x));
            // Deposits weigh more: nothing else can execute until there are funds.
            prop_oneof![3 => deposit, 2 => withdraw, 2 => transfer].boxed()
        } else {
            prop_oneof![3 => deposit, 2 => withdraw].boxed()
        }
    }

    fn apply(&mut self, env: &Env, action: &Action) -> Result<Outcome, String> {
        let client = VaultClient::new(env, &self.vault);
        match *action {
            Action::Deposit(a, amount) => {
                let a = self.actors.id(a);
                let x = amount.resolve(self.wallet_of(a));
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.wallet_of(a) < x {
                    Err(VaultError::InsufficientFunds)
                } else if self.deposit_of(a).checked_add(x).is_none()
                    || self.total().checked_add(x).is_none()
                {
                    Err(VaultError::Overflow)
                } else {
                    Ok(())
                };
                let got = client.try_deposit(self.actors.get(a), &x);
                let outcome = check_call(env, "deposit", got, expected)?;
                if outcome == Outcome::Executed {
                    *self.deposits.entry(a).or_default() += x;
                    *self.wallets.entry(a).or_default() -= x;
                }
                Ok(outcome)
            }
            Action::Withdraw(a, amount) => {
                let a = self.actors.id(a);
                let x = amount.resolve(self.deposit_of(a));
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.deposit_of(a) < x {
                    Err(VaultError::InsufficientBalance)
                } else {
                    Ok(())
                };
                let got = client.try_withdraw(self.actors.get(a), &x);
                let outcome = check_call(env, "withdraw", got, expected)?;
                if outcome == Outcome::Executed {
                    *self.deposits.entry(a).or_default() -= x;
                    *self.wallets.entry(a).or_default() += x;
                }
                Ok(outcome)
            }
            Action::Transfer(a, b, amount) => {
                let (a, b) = (self.actors.id(a), self.actors.id(b));
                let x = amount.resolve(self.deposit_of(a));
                let expected = if x <= 0 {
                    Err(VaultError::InvalidAmount)
                } else if self.deposit_of(a) < x {
                    Err(VaultError::InsufficientBalance)
                } else {
                    Ok(())
                };
                let got = client.try_transfer(self.actors.get(a), self.actors.get(b), &x);
                let outcome = check_call(env, "transfer", got, expected)?;
                if outcome == Outcome::Executed {
                    *self.deposits.entry(a).or_default() -= x;
                    *self.deposits.entry(b).or_default() += x;
                }
                Ok(outcome)
            }
        }
    }

    fn invariants(&self, env: &Env) -> Result<(), String> {
        let vault = VaultClient::new(env, &self.vault);
        let token = TokenClient::new(env, &self.token);
        for (id, addr) in self.actors.iter() {
            let (got, want) = (vault.balance(addr), self.deposit_of(id));
            if got != want {
                return Err(format!(
                    "vault balance of actor {}: contract {got}, model {want}",
                    id.0
                ));
            }
            let (got, want) = (token.balance(addr), self.wallet_of(id));
            if got != want {
                return Err(format!(
                    "token wallet of actor {}: {got}, model {want}",
                    id.0
                ));
            }
        }
        let (total, held) = (vault.total(), token.balance(&self.vault));
        if total != self.total() || held != self.total() {
            return Err(format!(
                "vault total {total} / tokens held {held}, model {}",
                self.total()
            ));
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
