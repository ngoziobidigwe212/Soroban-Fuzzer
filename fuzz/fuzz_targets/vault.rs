//! Coverage-guided fuzzing of the correct vault. Should run forever.
//!
//!     cargo +nightly fuzz run -s none vault -- -max_len=256 -len_control=0
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_fuzzer::Fuzzer;
use vault_example::{harness::VaultHarness, Vault};

fuzz_target!(|data: &[u8]| {
    Fuzzer::<VaultHarness<Vault>>::new()
        .check_unsigned_calls()
        .fuzz_one(data)
});
