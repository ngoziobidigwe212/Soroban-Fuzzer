//! The Stellar Asset Contract (the built-in token) against its model.
//! Should run without crashing.
//!
//!     cargo +nightly fuzz run -s none stellar_asset -- -max_len=256 -len_control=0
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_fuzzer::Fuzzer;
use stellar_asset_example::harness::SacHarness;

fuzz_target!(|data: &[u8]| Fuzzer::<SacHarness>::new().fuzz_one(data));
