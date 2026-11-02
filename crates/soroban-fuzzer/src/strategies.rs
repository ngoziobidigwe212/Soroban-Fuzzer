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
