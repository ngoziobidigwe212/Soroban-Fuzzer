mod bugs;
mod correct;
mod libfuzzer;
mod ttl;
mod wasm;

use soroban_fuzzer::prelude::*;

use crate::harness::{Variant, VaultHarness};

/// Seeds each bug must be found with, so detection never relies on luck.
pub const SEEDS: [u64; 3] = [1, 42, 0xdead_beef];

/// Run `fuzzer` and assert it finds a failure of `kind` that shrinks to at
/// most `max_steps` steps.
pub fn expect_bug<V: Variant>(
    kind: FailureKind,
    max_steps: usize,
    configure: impl Fn(Fuzzer<VaultHarness<V>>) -> Fuzzer<VaultHarness<V>>,
) {
    for seed in SEEDS {
        let fuzzer = configure(Fuzzer::new().seed(seed).cases(256));
        let err = fuzzer
            .run()
            .expect_err("the fuzzer should have found the bug");
        std::println!("{err}");
        assert_eq!(err.kind, kind, "wrong classification:\n{err}");
        assert!(
            err.steps.len() <= max_steps,
            "case was not shrunk well:\n{err}"
        );
    }
}
