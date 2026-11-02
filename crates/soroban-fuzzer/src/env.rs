use soroban_sdk::{
    testutils::{EnvTestConfig, Ledger},
    Env,
};

/// Seconds per ledger used when advancing time.
const SECONDS_PER_LEDGER: u64 = 5;

/// Build the `Env` used for one fuzz case.
pub(crate) fn fresh_env(mock_all_auths: bool) -> Env {
    // Snapshots would write a JSON file per case; never wanted while fuzzing.
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    if mock_all_auths {
        env.mock_all_auths();
    }
    env
}

/// Move the ledger forward by `ledgers` (sequence number and timestamp).
///
/// Storage entries whose TTL runs out become archived (persistent/instance)
/// or disappear (temporary), exactly as they would on the network.
pub fn advance_ledgers(env: &Env, ledgers: u32) {
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number.saturating_add(ledgers);
        l.timestamp = l
            .timestamp
            .saturating_add(ledgers as u64 * SECONDS_PER_LEDGER);
    });
}

/// How the fuzzer interleaves ledger time jumps with actions.
///
/// Time jumps expose storage lifecycle bugs: data kept in `temporary()`
/// storage vanishes, and persistent entries that are never extended get
/// archived, which makes the next call touching them fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedgerJumps {
    /// Relative weight of a jump step against an action step (which has a
    /// weight of 10). `0` disables jumps.
    pub weight: u32,
    /// Largest single jump, in ledgers.
    pub max_jump: u32,
    /// Total ledgers a single case may advance. A contract is expected to
    /// keep all of its state alive for this long without anyone touching it,
    /// so set it to the TTL your contract promises (default ≈ 7 days).
    pub horizon: u32,
}

impl LedgerJumps {
    pub const DISABLED: Self = Self {
        weight: 0,
        max_jump: 0,
        horizon: 0,
    };

    pub fn enabled(&self) -> bool {
        self.weight > 0 && self.max_jump > 0 && self.horizon > 0
    }
}

impl Default for LedgerJumps {
    fn default() -> Self {
        Self {
            weight: 2,
            max_jump: 50_000,
            horizon: 120_960, // 7 days of 5s ledgers
        }
    }
}
