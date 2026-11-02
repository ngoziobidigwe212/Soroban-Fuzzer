use std::fmt;

use crate::ResourceUsage;

/// Statistics from a run where every case passed.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub seed: u64,
    pub cases: u32,
    pub steps: u64,
    pub executed: u64,
    pub rejected: u64,
    pub skipped: u64,
    pub ledger_jumps: u64,
    /// Field-wise peak resource usage of any single invocation.
    pub peak: ResourceUsage,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "soroban-fuzzer: {} cases / {} steps passed (seed {})",
            self.cases, self.steps, self.seed
        )?;
        writeln!(
            f,
            "  calls: {} executed, {} rejected, {} skipped; {} ledger jumps",
            self.executed, self.rejected, self.skipped, self.ledger_jumps
        )?;
        write!(f, "  peak per call: {}", self.peak)
    }
}
