//! The TTL helpers read storage without restoring archived entries.

mod common;

use common::{vault::Vault, DataKey, VaultClient, BUMP_TO};
use soroban_fuzzer::{advance_ledgers, storage, ttl, Actors};
use soroban_sdk::{testutils::EnvTestConfig, Env};

fn env() -> Env {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    env
}

#[test]
fn reports_and_counts_down_ttls() {
    let env = env();
    let id = env.register(Vault, ());
    let user = Actors::generate(&env, 1)
        .get(soroban_fuzzer::ActorId(0))
        .clone();
    let key = DataKey::Balance(user.clone());

    assert_eq!(ttl::persistent(&env, &id, &key), None);
    VaultClient::new(&env, &id).deposit(&user, &10);
    assert_eq!(ttl::persistent(&env, &id, &key), Some(BUMP_TO));
    assert_eq!(ttl::instance(&env, &id), Some(BUMP_TO));

    advance_ledgers(&env, 1_000);
    assert_eq!(ttl::persistent(&env, &id, &key), Some(BUMP_TO - 1_000));
    assert_eq!(ttl::temporary(&env, &id, &key), None);
}

#[test]
fn expired_entries_stay_expired_when_observed() {
    let env = env();
    let id = env.register(Vault, ());
    let user = Actors::generate(&env, 1)
        .get(soroban_fuzzer::ActorId(0))
        .clone();
    VaultClient::new(&env, &id).deposit(&user, &10);
    assert!(storage::expired(&env, None).is_empty());

    advance_ledgers(&env, BUMP_TO + 1);
    let key = DataKey::Balance(user);
    // Reading the TTL twice must not restore the entry.
    assert_eq!(ttl::persistent(&env, &id, &key), Some(0));
    assert_eq!(ttl::persistent(&env, &id, &key), Some(0));
    let expired = storage::expired(&env, Some(std::slice::from_ref(&id)));
    assert_eq!(
        expired.len(),
        2,
        "balance and instance should be expired: {expired:?}"
    );
}
