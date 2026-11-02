//! Ready-made proptest strategies for values Soroban contracts commonly take.

use proptest::prelude::*;

use crate::ActorId;

/// Any actor in a pool of `n` actors.
pub fn actor(n: u8) -> impl Strategy<Value = ActorId> + Clone {
    assert!(n > 0, "actor pool must not be empty");
    (0..n).prop_map(ActorId)
}

// Note on ordering: proptest shrinks a `prop_oneof!` towards its *first*
// branch, so the plain small-number range comes first. Failing cases then
// shrink to readable values like `1` instead of `i128::MAX - 1`.

/// Any `i128`, biased towards the values that break token arithmetic:
/// zero, ±1, negatives, `i128::MIN/MAX` and their neighbours.
pub fn amount() -> impl Strategy<Value = i128> + Clone {
    prop_oneof![
        5 => 0i128..1_000_000,
        1 => -1_000i128..0,
        3 => prop::sample::select(vec![
            0i128,
            1,
            -1,
            i128::MAX,
            i128::MIN,
            i128::MAX - 1,
            i128::MIN + 1,
            i64::MAX as i128,
            u64::MAX as i128,
        ]),
        2 => any::<i128>(),
    ]
}

/// A non-negative amount up to `max`, biased towards `0`, `1` and `max`.
pub fn amount_up_to(max: i128) -> impl Strategy<Value = i128> + Clone {
    assert!(max >= 0);
    prop_oneof![
        6 => 0..=max,
        1 => prop::sample::select(vec![0, 1.min(max), max]),
    ]
}

/// A strictly positive amount up to `max`.
pub fn positive_amount(max: i128) -> impl Strategy<Value = i128> + Clone {
    assert!(max >= 1);
    prop_oneof![
        6 => 1..=max,
        1 => prop::sample::select(vec![1, max]),
    ]
}

/// A ledger jump between 1 and `max` ledgers, mixing short and long gaps.
pub fn ledger_jump(max: u32) -> impl Strategy<Value = u32> + Clone {
    let max = max.max(1);
    prop_oneof![
        3 => 1..=max.min(100),
        2 => 1..=max.min(10_000),
        2 => 1..=max,
        1 => Just(max),
    ]
}

/// A short ASCII string valid as a Soroban `Symbol` (`[a-zA-Z0-9_]{1,32}`).
pub fn symbol_str() -> impl Strategy<Value = String> + Clone {
    "[a-zA-Z0-9_]{1,32}"
}

/// Arbitrary bytes, up to `max_len` long.
pub fn bytes(max_len: usize) -> impl Strategy<Value = Vec<u8>> + Clone {
    prop::collection::vec(any::<u8>(), 0..=max_len)
}

/// An amount described relative to what is available at the time the action
/// runs, e.g. "all of the sender's balance" or "balance + 1".
///
/// Actions are generated before they run, so a plain random amount almost
/// never matches the actual balance: most withdrawals and transfers are then
/// rejected and the interesting "exactly enough" and "one too many" cases are
/// rare. Generate `Amount`s with [`relative_amount`] and turn them into
/// numbers with [`Amount::resolve`] inside `apply`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    /// Everything available.
    All,
    /// A percentage (1..=100) of what is available, rounded down.
    Percent(u8),
    /// One more than what is available: should always be rejected.
    AllPlusOne,
    /// A fixed value from [`amount`], independent of the state.
    Exact(i128),
}

impl Amount {
    /// The concrete amount given the `available` balance.
    pub fn resolve(self, available: i128) -> i128 {
        match self {
            Amount::All => available,
            Amount::Percent(p) => available / 100 * p as i128 + available % 100 * p as i128 / 100,
            Amount::AllPlusOne => available.saturating_add(1),
            Amount::Exact(x) => x,
        }
    }
}

/// Strategy for [`Amount`]: mostly state-relative amounts, plus fixed edge
/// values from [`amount`]. Shrinks towards [`Amount::All`].
pub fn relative_amount() -> impl Strategy<Value = Amount> + Clone {
    prop_oneof![
        3 => Just(Amount::All),
        5 => (1u8..=100).prop_map(Amount::Percent),
        1 => Just(Amount::AllPlusOne),
        1 => amount().prop_map(Amount::Exact),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{
        strategy::ValueTree,
        test_runner::{Config, TestRunner},
    };

    fn sample<S: Strategy>(s: S, n: usize) -> Vec<S::Value> {
        let mut runner = TestRunner::new(Config::default());
        (0..n)
            .map(|_| s.new_tree(&mut runner).unwrap().current())
            .collect()
    }

    #[test]
    fn amount_hits_edge_values() {
        let values = sample(amount(), 2_000);
        for edge in [0, 1, -1, i128::MAX, i128::MIN] {
            assert!(values.contains(&edge), "{edge} never generated");
        }
    }

    #[test]
    fn bounded_amounts_stay_in_range() {
        assert!(sample(amount_up_to(50), 500)
            .iter()
            .all(|x| (0..=50).contains(x)));
        assert!(sample(positive_amount(50), 500)
            .iter()
            .all(|x| (1..=50).contains(x)));
        assert!(sample(ledger_jump(10), 500)
            .iter()
            .all(|x| (1..=10).contains(x)));
        assert!(sample(actor(3), 500).iter().all(|a| a.0 < 3));
    }

    #[test]
    fn relative_amounts_resolve() {
        assert_eq!(Amount::All.resolve(70), 70);
        assert_eq!(Amount::AllPlusOne.resolve(70), 71);
        assert_eq!(Amount::AllPlusOne.resolve(i128::MAX), i128::MAX);
        assert_eq!(Amount::Percent(50).resolve(70), 35);
        assert_eq!(Amount::Percent(100).resolve(i128::MAX), i128::MAX);
        assert_eq!(Amount::Percent(1).resolve(99), 0);
        assert_eq!(Amount::Exact(-5).resolve(70), -5);
    }

    #[test]
    fn symbols_are_valid() {
        for s in sample(symbol_str(), 200) {
            assert!((1..=32).contains(&s.len()));
            assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        }
    }
}
