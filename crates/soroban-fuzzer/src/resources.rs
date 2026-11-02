use std::fmt;

use soroban_sdk::Env;

/// Resources metered for one top-level contract invocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceUsage {
    pub instructions: i64,
    pub mem_bytes: i64,
    /// Entries read from disk (archived/classic entries).
    pub disk_read_entries: u32,
    /// Live Soroban entries accessed in memory.
    pub memory_read_entries: u32,
    pub write_entries: u32,
    pub disk_read_bytes: u32,
    pub write_bytes: u32,
    pub contract_events_size_bytes: u32,
}

impl ResourceUsage {
    /// Usage of the last top-level invocation in `env`, if any call was made.
    pub fn last(env: &Env) -> Option<Self> {
        env.host()
            .get_last_invocation_resources()
            .map(|r| ResourceUsage {
                instructions: r.instructions,
                mem_bytes: r.mem_bytes,
                disk_read_entries: r.disk_read_entries,
                memory_read_entries: r.memory_read_entries,
                write_entries: r.write_entries,
                disk_read_bytes: r.disk_read_bytes,
                write_bytes: r.write_bytes,
                contract_events_size_bytes: r.contract_events_size_bytes,
            })
    }

    /// Every ledger entry the invocation had to declare in its footprint.
    pub fn footprint_entries(&self) -> u32 {
        self.disk_read_entries + self.memory_read_entries
    }

    /// Field-wise maximum, used to track peak usage across a run.
    pub fn max(self, o: Self) -> Self {
        Self {
            instructions: self.instructions.max(o.instructions),
            mem_bytes: self.mem_bytes.max(o.mem_bytes),
            disk_read_entries: self.disk_read_entries.max(o.disk_read_entries),
            memory_read_entries: self.memory_read_entries.max(o.memory_read_entries),
            write_entries: self.write_entries.max(o.write_entries),
            disk_read_bytes: self.disk_read_bytes.max(o.disk_read_bytes),
            write_bytes: self.write_bytes.max(o.write_bytes),
            contract_events_size_bytes: self
                .contract_events_size_bytes
                .max(o.contract_events_size_bytes),
        }
    }
}

impl fmt::Display for ResourceUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cpu={} mem={}B footprint={} (disk={}, mem={}) writes={} read={}B written={}B events={}B",
            self.instructions,
            self.mem_bytes,
            self.footprint_entries(),
            self.disk_read_entries,
            self.memory_read_entries,
            self.write_entries,
            self.disk_read_bytes,
            self.write_bytes,
            self.contract_events_size_bytes,
        )
    }
}

/// Upper bounds a single invocation may use before the fuzzer reports it.
///
/// The test `Env` already panics when a call exceeds the *full* mainnet
/// limits. This budget is stricter on purpose: a call that uses 90% of the
/// footprint with 5 users will fail once there are 6. By default it is set to
/// 75% of mainnet limits ([`ResourceBudget::default`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceBudget {
    pub max_instructions: i64,
    pub max_mem_bytes: i64,
    /// Total entries in the footprint (disk + in-memory reads).
    pub max_footprint_entries: u32,
    pub max_disk_read_entries: u32,
    pub max_write_entries: u32,
    pub max_disk_read_bytes: u32,
    pub max_write_bytes: u32,
    pub max_contract_events_size_bytes: u32,
}

impl ResourceBudget {
    /// Stellar mainnet per-transaction limits (snapshot shipped with
    /// soroban-sdk 29).
    pub const MAINNET: Self = Self {
        max_instructions: 400_000_000,
        max_mem_bytes: 41_943_040,
        max_footprint_entries: 400,
        max_disk_read_entries: 200,
        max_write_entries: 200,
        max_disk_read_bytes: 200_000,
        max_write_bytes: 132_096,
        max_contract_events_size_bytes: 16_384,
    };

    /// No limits; only the `Env`'s own enforcement applies.
    pub const UNLIMITED: Self = Self {
        max_instructions: i64::MAX,
        max_mem_bytes: i64::MAX,
        max_footprint_entries: u32::MAX,
        max_disk_read_entries: u32::MAX,
        max_write_entries: u32::MAX,
        max_disk_read_bytes: u32::MAX,
        max_write_bytes: u32::MAX,
        max_contract_events_size_bytes: u32::MAX,
    };

    /// Mainnet limits scaled to `percent` (1..=100).
    pub fn mainnet_percent(percent: u32) -> Self {
        let p = percent.clamp(1, 100);
        let m = Self::MAINNET;
        let s32 = |v: u32| (v as u64 * p as u64 / 100) as u32;
        let s64 = |v: i64| v / 100 * p as i64;
        Self {
            max_instructions: s64(m.max_instructions),
            max_mem_bytes: s64(m.max_mem_bytes),
            max_footprint_entries: s32(m.max_footprint_entries),
            max_disk_read_entries: s32(m.max_disk_read_entries),
            max_write_entries: s32(m.max_write_entries),
            max_disk_read_bytes: s32(m.max_disk_read_bytes),
            max_write_bytes: s32(m.max_write_bytes),
            max_contract_events_size_bytes: s32(m.max_contract_events_size_bytes),
        }
    }

    /// Returns a description of every exceeded limit, or `None`.
    pub fn check(&self, u: &ResourceUsage) -> Option<String> {
        let mut over = Vec::new();
        let mut cmp = |name: &str, used: i128, max: i128| {
            if used > max {
                over.push(format!("{name}: {used} > {max}"));
            }
        };
        cmp(
            "instructions",
            u.instructions as i128,
            self.max_instructions as i128,
        );
        cmp("mem_bytes", u.mem_bytes as i128, self.max_mem_bytes as i128);
        cmp(
            "footprint_entries",
            u.footprint_entries() as i128,
            self.max_footprint_entries as i128,
        );
        cmp(
            "disk_read_entries",
            u.disk_read_entries as i128,
            self.max_disk_read_entries as i128,
        );
        cmp(
            "write_entries",
            u.write_entries as i128,
            self.max_write_entries as i128,
        );
        cmp(
            "disk_read_bytes",
            u.disk_read_bytes as i128,
            self.max_disk_read_bytes as i128,
        );
        cmp(
            "write_bytes",
            u.write_bytes as i128,
            self.max_write_bytes as i128,
        );
        cmp(
            "contract_events_size_bytes",
            u.contract_events_size_bytes as i128,
            self.max_contract_events_size_bytes as i128,
        );
        (!over.is_empty()).then(|| over.join(", "))
    }
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::mainnet_percent(75)
    }
}
