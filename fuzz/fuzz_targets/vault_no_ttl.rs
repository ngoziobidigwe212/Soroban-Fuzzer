//! A vault that never extends its balances' TTL. libFuzzer should report a
//! crash (an `ArchivedEntry` failure) within seconds.
//!
//!     cargo +nightly fuzz run vault_no_ttl
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_fuzzer::Fuzzer;
use vault_example::{buggy::no_ttl::NoTtlVault, harness::VaultHarness};

fuzz_target!(|data: &[u8]| Fuzzer::<VaultHarness<NoTtlVault>>::new().fuzz_one(data));
