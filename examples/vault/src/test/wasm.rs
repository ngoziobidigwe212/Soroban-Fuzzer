//! Fuzz the compiled Wasm, so VM instantiation and execution costs are part
//! of the measured resources.

use soroban_fuzzer::prelude::*;

use crate::{
    harness::{VaultHarness, VaultWasm, VAULT_WASM},
    Vault,
};

/// The repository ships an empty placeholder for `fixtures/vault.wasm`.
/// Without the real build these tests skip, so `cargo test` still works
/// without stellar-cli. CI always builds the fixture first.
fn fixture_missing() -> bool {
    if VAULT_WASM.is_empty() {
        std::eprintln!(
            "skipping: fixtures/vault.wasm is empty, build it with `stellar contract build`"
        );
    }
    VAULT_WASM.is_empty()
}

#[test]
fn wasm_vault_passes() {
    if fixture_missing() {
        return;
    }
    let report = Fuzzer::<VaultHarness<VaultWasm>>::new()
        .cases(48)
        .check_unsigned_calls()
        .check();
    assert!(report.executed > 0, "{report}");
}

#[test]
fn wasm_costs_more_than_native() {
    if fixture_missing() {
        return;
    }
    let run = |native: bool| {
        let f = |seed| {
            if native {
                Fuzzer::<VaultHarness<Vault>>::new()
                    .seed(seed)
                    .cases(16)
                    .run()
                    .unwrap()
            } else {
                Fuzzer::<VaultHarness<VaultWasm>>::new()
                    .seed(seed)
                    .cases(16)
                    .run()
                    .unwrap()
            }
        };
        f(11).peak.instructions
    };
    let (native, wasm) = (run(true), run(false));
    assert!(
        wasm > native * 2,
        "Wasm execution should be metered: native {native}, wasm {wasm}"
    );
}
