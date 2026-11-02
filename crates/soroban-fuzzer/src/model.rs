//! Small building blocks for harness models, so a harness can stay focused
//! on the contract's rules instead of bookkeeping.

use std::collections::BTreeMap;

use soroban_sdk::{
    testutils::{Address as _, StellarAssetContract},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::{ActorId, Actors};

/// Per-actor amounts, e.g. token balances or deposits. Missing actors hold 0.
///
/// The arithmetic panics on overflow: the model is meant to mirror amounts
/// the contract accepted, so an overflow here means the harness itself is
/// wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Balances(BTreeMap<ActorId, i128>);

impl Balances {
    pub fn new() -> Self {
        Self::default()
    }

    /// The amount held by `a` (0 if never set).
    pub fn get(&self, a: ActorId) -> i128 {
        self.0.get(&a).copied().unwrap_or(0)
    }

    pub fn set(&mut self, a: ActorId, amount: i128) {
        self.0.insert(a, amount);
    }

    /// Add `amount` to `a`.
    pub fn credit(&mut self, a: ActorId, amount: i128) {
        let v = self.get(a).checked_add(amount).expect("model overflow");
        self.set(a, v);
    }

    /// Take `amount` from `a`.
    pub fn debit(&mut self, a: ActorId, amount: i128) {
        let v = self.get(a).checked_sub(amount).expect("model underflow");
        self.set(a, v);
    }

    /// Move `amount` from `from` to `to`.
    pub fn transfer(&mut self, from: ActorId, to: ActorId, amount: i128) {
        self.debit(from, amount);
        self.credit(to, amount);
    }

    /// Sum over all actors.
    pub fn total(&self) -> i128 {
        self.0
            .values()
            .try_fold(0i128, |acc, v| acc.checked_add(*v))
            .expect("model total overflows i128")
    }

    /// The actors with a recorded amount and their amounts.
    pub fn iter(&self) -> impl Iterator<Item = (ActorId, i128)> + '_ {
        self.0.iter().map(|(a, v)| (*a, *v))
    }

    /// Compare against what the contract reports for every actor, e.g.
    /// `model.check(&actors, "vault balance", |a| vault.balance(a))`.
    pub fn check(
        &self,
        actors: &Actors,
        what: &str,
        mut read: impl FnMut(&Address) -> i128,
    ) -> Result<(), String> {
        for (id, addr) in actors.iter() {
            let (got, want) = (read(addr), self.get(id));
            if got != want {
                return Err(format!(
                    "{what} of actor {}: contract {got}, model {want}",
                    id.0
                ));
            }
        }
        Ok(())
    }
}

/// The error your model expects for a call: the first rule whose condition
/// holds, or `Ok(())` if none does. Pass the result to
/// [`check_call`](crate::check_call).
///
/// ```ignore
/// let expected = expect_unless([
///     (x <= 0, Error::InvalidAmount),
///     (balance < x, Error::InsufficientBalance),
/// ]);
/// ```
pub fn expect_unless<E>(rules: impl IntoIterator<Item = (bool, E)>) -> Result<(), E> {
    match rules.into_iter().find(|(cond, _)| *cond) {
        Some((_, e)) => Err(e),
        None => Ok(()),
    }
}

/// Return `Err` from the enclosing function when two values differ, with a
/// message showing both: `ensure_eq!(contract_value, model_value, "total")`.
///
/// Meant for [`Harness::invariants`](crate::Harness::invariants) and
/// [`Harness::apply`](crate::Harness::apply), which return
/// `Result<_, String>`.
#[macro_export]
macro_rules! ensure_eq {
    ($got:expr, $want:expr, $($what:tt)+) => {{
        let (got, want) = (&$got, &$want);
        if got != want {
            return ::core::result::Result::Err(::std::format!(
                "{}: contract {:?}, model {:?}",
                ::std::format_args!($($what)+),
                got,
                want
            ));
        }
    }};
}

/// A Stellar Asset Contract (the standard token) deployed for a test, with
/// its admin. Many contracts hold or move tokens; this sets one up in a line.
#[derive(Clone, Debug)]
pub struct TestToken {
    pub address: Address,
    pub admin: Address,
}

impl TestToken {
    /// Deploy a fresh Stellar Asset Contract with a generated admin.
    pub fn deploy(env: &Env) -> Self {
        let admin = Address::generate(env);
        Self::from_sac(env.register_stellar_asset_contract_v2(admin.clone()), admin)
    }

    /// Wrap a contract made with `env.register_stellar_asset_contract_v2(admin)`,
    /// e.g. after setting issuer flags on it.
    pub fn from_sac(sac: StellarAssetContract, admin: Address) -> Self {
        Self {
            address: sac.address(),
            admin,
        }
    }

    /// Mint `amount` to every actor and return the matching model.
    pub fn mint_to_all(&self, env: &Env, actors: &Actors, amount: i128) -> Balances {
        let admin = self.admin_client(env);
        let mut model = Balances::new();
        for (id, addr) in actors.iter() {
            admin.mint(addr, &amount);
            model.set(id, amount);
        }
        model
    }

    /// The token interface: `balance`, `transfer`, `approve`, …
    pub fn client<'a>(&self, env: &Env) -> TokenClient<'a> {
        TokenClient::new(env, &self.address)
    }

    /// The admin interface: `mint`, `clawback`, `set_authorized`, …
    pub fn admin_client<'a>(&self, env: &Env) -> StellarAssetClient<'a> {
        StellarAssetClient::new(env, &self.address)
    }

    /// Check every actor's token balance against `model`.
    pub fn check_balances(
        &self,
        env: &Env,
        actors: &Actors,
        model: &Balances,
    ) -> Result<(), String> {
        let token = self.client(env);
        model.check(actors, "token balance", |a| token.balance(a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balances_track_amounts() {
        let (a, b) = (ActorId(0), ActorId(1));
        let mut m = Balances::new();
        m.credit(a, 10);
        m.transfer(a, b, 4);
        assert_eq!((m.get(a), m.get(b), m.get(ActorId(2))), (6, 4, 0));
        assert_eq!(m.total(), 10);
    }

    #[test]
    fn expect_unless_returns_first_matching_rule() {
        assert_eq!(expect_unless([(false, 1), (true, 2), (true, 3)]), Err(2));
        assert_eq!(expect_unless::<u8>([(false, 1)]), Ok(()));
    }

    #[test]
    fn ensure_eq_reports_both_values() {
        fn f(x: i32) -> Result<(), String> {
            ensure_eq!(x, 2, "value of {}", "x");
            Ok(())
        }
        assert_eq!(f(2), Ok(()));
        assert_eq!(f(3).unwrap_err(), "value of x: contract 3, model 2");
    }

    #[test]
    fn test_token_mints_and_checks() {
        let env = Env::default();
        env.mock_all_auths();
        let token = TestToken::deploy(&env);
        let actors = Actors::generate(&env, 2);
        let mut model = token.mint_to_all(&env, &actors, 100);
        token.check_balances(&env, &actors, &model).unwrap();
        assert_eq!(token.admin_client(&env).admin(), token.admin);
        model.credit(ActorId(1), 1);
        assert!(token.check_balances(&env, &actors, &model).is_err());
    }
}
