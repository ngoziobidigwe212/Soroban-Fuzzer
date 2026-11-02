//! The TTL helpers read storage without restoring archived entries.

use soroban_fuzzer::{advance_ledgers, storage, ttl, ActorId, Actors};
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::StellarAssetClient,
    Address, Env,
};

use crate::{DataKey, Vault, VaultClient, BUMP_TO};

fn setup() -> (Env, Address, Address) {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let vault = env.register(Vault, (token.clone(),));
    let user = Actors::generate(&env, 1).get(ActorId(0)).clone();
    StellarAssetClient::new(&env, &token).mint(&user, &1_000);
    VaultClient::new(&env, &vault).deposit(&user, &10);
    (env, vault, user)
}

#[test]
fn reports_and_counts_down_ttls() {
    let (env, vault, user) = setup();
    let key = DataKey::Balance(user);
    assert_eq!(ttl::persistent(&env, &vault, &key), Some(BUMP_TO));
    assert_eq!(ttl::instance(&env, &vault), Some(BUMP_TO));
    assert_eq!(
        ttl::persistent(&env, &vault, &DataKey::Balance(Address::generate(&env))),
        None
    );

    advance_ledgers(&env, 1_000);
    assert_eq!(ttl::persistent(&env, &vault, &key), Some(BUMP_TO - 1_000));
    assert_eq!(ttl::temporary(&env, &vault, &key), None);
}

#[test]
fn expired_entries_stay_expired_when_observed() {
    let (env, vault, user) = setup();
    assert!(storage::expired(&env, None).is_empty());

    advance_ledgers(&env, BUMP_TO + 1);
    let key = DataKey::Balance(user);
    // Reading the TTL twice must not restore the entry.
    assert_eq!(ttl::persistent(&env, &vault, &key), Some(0));
    assert_eq!(ttl::persistent(&env, &vault, &key), Some(0));
    let expired = storage::expired(&env, Some(std::slice::from_ref(&vault)));
    assert_eq!(
        expired.len(),
        2,
        "balance and instance should be expired: {expired:?}"
    );
}
