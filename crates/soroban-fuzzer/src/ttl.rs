//! Read storage TTLs from outside a contract, for use in invariants.
//!
//! These helpers read the raw test ledger, so unlike the SDK's `get_ttl` they
//! never restore an archived entry as a side effect. Each returns the number
//! of ledgers the entry has left (`live_until - current_sequence`), `Some(0)`
//! for an expired (archived) persistent entry, or `None` if there is no such
//! entry. Expired temporary entries count as deleted and return `None`.

use soroban_sdk::{Address, Env, IntoVal, Val};

use crate::storage::{find, to_scval, Durability};

/// TTL of a persistent entry of `contract`.
pub fn persistent<K: IntoVal<Env, Val>>(env: &Env, contract: &Address, key: &K) -> Option<u32> {
    let key = to_scval(env, key);
    find(env, contract, Some(&key), Durability::Persistent).map(|e| e.ttl(env.ledger().sequence()))
}

/// TTL of a temporary entry of `contract`.
pub fn temporary<K: IntoVal<Env, Val>>(env: &Env, contract: &Address, key: &K) -> Option<u32> {
    let seq = env.ledger().sequence();
    let key = to_scval(env, key);
    find(env, contract, Some(&key), Durability::Temporary)
        .filter(|e| !e.is_expired(seq))
        .map(|e| e.ttl(seq))
}

/// TTL of the contract instance (and its instance storage).
pub fn instance(env: &Env, contract: &Address) -> Option<u32> {
    find(env, contract, None, Durability::Instance).map(|e| e.ttl(env.ledger().sequence()))
}
