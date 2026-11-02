//! Side-effect free view of the ledger entries held by the test `Env`.
//!
//! Reading storage through `env.storage()` (or the SDK's `get_ttl`) touches
//! the entry, and since protocol 23 touching an archived entry silently
//! *restores* it. To observe expiry without changing it, the fuzzer reads the
//! host's raw storage map instead.

use std::fmt;

use soroban_sdk::{
    xdr::{ContractDataDurability, LedgerKey, ScAddress, ScVal},
    Address, Env, TryFromVal,
};

/// Storage class of a contract ledger entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    Persistent,
    Temporary,
    /// The contract instance entry (holds instance storage).
    Instance,
    /// Contract code (Wasm). Native test contracts have one too.
    Code,
}

/// One contract-related entry in the test ledger.
#[derive(Clone)]
pub struct StoredEntry {
    /// Owning contract (`None` for code entries).
    pub contract: Option<ScAddress>,
    /// Storage key (`None` for code entries).
    pub key: Option<ScVal>,
    pub durability: Durability,
    /// Last ledger (inclusive) on which the entry is live.
    pub live_until: u32,
}

impl StoredEntry {
    /// `true` once the entry's TTL has run out at ledger `seq`.
    pub fn is_expired(&self, seq: u32) -> bool {
        self.live_until < seq
    }

    /// Ledgers left to live at `seq` (`0` when expired).
    pub fn ttl(&self, seq: u32) -> u32 {
        self.live_until.saturating_sub(seq)
    }

    pub fn belongs_to(&self, contract: &Address) -> bool {
        self.contract.as_ref() == Some(&ScAddress::from(contract))
    }
}

impl fmt::Debug for StoredEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} entry", self.durability)?;
        if let Some(c) = &self.contract {
            write!(f, " of {c}")?;
        }
        if let Some(k) = &self.key {
            if self.durability != Durability::Instance {
                write!(f, " key={}", short_scval(k))?;
            }
        }
        write!(f, " (live until ledger {})", self.live_until)
    }
}

/// All contract data and code entries currently in the ledger.
pub fn entries(env: &Env) -> Vec<StoredEntry> {
    let stored = env
        .host()
        .get_stored_entries()
        .expect("reading test ledger storage");
    stored
        .iter()
        .filter_map(|(key, value)| {
            let live_until = value.as_ref().and_then(|(_, ttl)| *ttl)?;
            match key.as_ref() {
                LedgerKey::ContractData(d) => {
                    let durability = match (&d.key, d.durability) {
                        (ScVal::LedgerKeyContractInstance, _) => Durability::Instance,
                        (_, ContractDataDurability::Persistent) => Durability::Persistent,
                        (_, ContractDataDurability::Temporary) => Durability::Temporary,
                    };
                    Some(StoredEntry {
                        contract: Some(d.contract.clone()),
                        key: Some(d.key.clone()),
                        durability,
                        live_until,
                    })
                }
                LedgerKey::ContractCode(_) => Some(StoredEntry {
                    contract: None,
                    key: None,
                    durability: Durability::Code,
                    live_until,
                }),
                _ => None,
            }
        })
        .collect()
}

/// Archivable entries (persistent, instance, code) whose TTL has run out.
///
/// When `contracts` is `Some`, only data entries of those contracts are
/// considered (code entries are skipped as they are shared).
pub fn expired(env: &Env, contracts: Option<&[Address]>) -> Vec<StoredEntry> {
    let seq = env.ledger().sequence();
    entries(env)
        .into_iter()
        .filter(|e| e.durability != Durability::Temporary && e.is_expired(seq))
        .filter(|e| match contracts {
            None => true,
            Some(cs) => cs.iter().any(|c| e.belongs_to(c)),
        })
        .collect()
}

/// Find the entry for `key` of `contract` with the given durability.
pub(crate) fn find(
    env: &Env,
    contract: &Address,
    key: Option<&ScVal>,
    durability: Durability,
) -> Option<StoredEntry> {
    entries(env).into_iter().find(|e| {
        e.durability == durability
            && e.belongs_to(contract)
            && (key.is_none() || e.key.as_ref() == key)
    })
}

/// Convert any contract value into its XDR form for key comparisons.
pub(crate) fn to_scval<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) -> ScVal {
    ScVal::try_from_val(env, &key.into_val(env)).expect("storage key converts to ScVal")
}

fn short_scval(v: &ScVal) -> String {
    let s = format!("{v:?}");
    if s.len() > 160 {
        format!("{}…", &s[..160])
    } else {
        s
    }
}
