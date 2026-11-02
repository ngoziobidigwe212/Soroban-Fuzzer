//! `fuzz_one` turns raw bytes into a case. These tests drive it with plain
//! pseudo-random bytes; the cargo-fuzz targets in `/fuzz` drive it with
//! libFuzzer's coverage-guided inputs.

use std::vec::Vec;

use soroban_fuzzer::{catch_quiet, prelude::*};

use crate::{buggy::no_ttl::NoTtlVault, harness::VaultHarness, Vault};

/// Deterministic pseudo-random byte strings (xorshift).
fn inputs(n: usize, len: usize) -> impl Iterator<Item = Vec<u8>> {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    (0..n).map(move |_| {
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    })
}

#[test]
fn correct_vault_never_crashes() {
    let fuzzer = Fuzzer::<VaultHarness<Vault>>::new();
    fuzzer.fuzz_one(&[]);
    fuzzer.fuzz_one(&[0xff; 7]);
    for data in inputs(64, 512) {
        fuzzer.fuzz_one(&data);
    }
}

#[test]
fn buggy_vault_crashes_on_some_input() {
    let fuzzer = Fuzzer::<VaultHarness<NoTtlVault>>::new();
    let crash = inputs(256, 512)
        .filter_map(|data| catch_quiet(|| fuzzer.fuzz_one(&data)).err())
        .next()
        .expect("random inputs should hit the missing-TTL bug");
    assert!(crash.contains("ArchivedEntry"), "{crash}");
}
