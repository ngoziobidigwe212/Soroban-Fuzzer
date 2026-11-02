//! The soroban-fuzzer harness for the vault.
//!
//! It keeps a model of every actor's vault balance and token wallet, and
//! checks the contract (and the real Stellar Asset Contract holding the
//! funds) against it after every step. The harness is generic over
//! [`Variant`], so the same model drives the correct vault, its compiled
//! Wasm, and each buggy variant.

use std::{marker::PhantomData, string::String, vec, vec::Vec};

use soroban_fuzzer::prelude::*;

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
    pub token: TestToken,
    pub actors: Actors,
    /// Model: each actor's balance inside the vault.
    pub deposits: Balances,
    /// Model: each actor's token wallet.
    pub wallets: Balances,
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

impl<V: Variant> Harness for VaultHarness<V> {
    type Action = Action;

    fn setup(env: &Env) -> Self {
        let token = TestToken::deploy(env);
        let actors = Actors::generate(env, ACTORS as usize);
        Self {
            vault: V::register(env, &token.address),
            wallets: token.mint_to_all(env, &actors, WALLET),
            token,
            actors,
            deposits: Balances::new(),
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
        let id = |a| self.actors.id(a);
        let addr = |a| self.actors.get(a);
        match *action {
            Action::Deposit(a, amount) => {
                let (a, wallet) = (id(a), self.wallets.get(id(a)));
                let x = amount.resolve(wallet);
                let expected = expect_unless([
                    (x <= 0, VaultError::InvalidAmount),
                    (wallet < x, VaultError::InsufficientFunds),
                    (
                        self.deposits.get(a).checked_add(x).is_none()
                            || self.deposits.total().checked_add(x).is_none(),
                        VaultError::Overflow,
                    ),
                ]);
                let got = client.try_deposit(addr(a), &x);
                Ok(check_call(env, "deposit", got, expected)?.if_executed(|| {
                    self.wallets.debit(a, x);
                    self.deposits.credit(a, x);
                }))
            }
            Action::Withdraw(a, amount) => {
                let (a, held) = (id(a), self.deposits.get(id(a)));
                let x = amount.resolve(held);
                let expected = expect_unless([
                    (x <= 0, VaultError::InvalidAmount),
                    (held < x, VaultError::InsufficientBalance),
                ]);
                let got = client.try_withdraw(addr(a), &x);
                Ok(check_call(env, "withdraw", got, expected)?.if_executed(|| {
                    self.deposits.debit(a, x);
                    self.wallets.credit(a, x);
                }))
            }
            Action::Transfer(a, b, amount) => {
                let (a, b, held) = (id(a), id(b), self.deposits.get(id(a)));
                let x = amount.resolve(held);
                let expected = expect_unless([
                    (x <= 0, VaultError::InvalidAmount),
                    (held < x, VaultError::InsufficientBalance),
                ]);
                let got = client.try_transfer(addr(a), addr(b), &x);
                Ok(check_call(env, "transfer", got, expected)?
                    .if_executed(|| self.deposits.transfer(a, b, x)))
            }
        }
    }

    fn invariants(&self, env: &Env) -> Result<(), String> {
        let vault = VaultClient::new(env, &self.vault);
        self.deposits
            .check(&self.actors, "vault balance", |a| vault.balance(a))?;
        self.token
            .check_balances(env, &self.actors, &self.wallets)?;
        let total = self.deposits.total();
        ensure_eq!(vault.total(), total, "vault total");
        ensure_eq!(
            self.token.client(env).balance(&self.vault),
            total,
            "tokens held by the vault"
        );
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
