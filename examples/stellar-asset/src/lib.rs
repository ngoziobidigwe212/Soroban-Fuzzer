//! A soroban-fuzzer harness for the **Stellar Asset Contract** (SAC): the
//! token contract built into the Soroban host that every Stellar asset uses.
//!
//! Unlike the vault example, this harness targets production code that the
//! fuzzer's author did not write. It models the SAC's documented rules
//! (balances, allowances that expire, deauthorization and clawback) and
//! checks the real contract against that model across ledger time jumps.
//! A clean run therefore shows two things: the fuzzer raises no false alarms
//! on a real, heavily used contract, and the model matches the SAC exactly.

pub mod harness;

#[cfg(test)]
mod test;
