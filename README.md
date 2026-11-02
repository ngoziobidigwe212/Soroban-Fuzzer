# Soroban-Fuzzer

Property-based fuzzing harness for Soroban smart contracts, running in the `soroban-sdk` test environment.

This repository holds the fuzzing part of the **soroban-guard** safety suite. The other planned parts are typed storage wrappers, a security linter and a rent-keeper daemon.

```
.
├── crates/
│   └── soroban-fuzzer/      # the fuzzing library (see its README)
├── examples/
│   └── vault/               # token vault, its harness, 5 buggy variants and the tests
├── fuzz/                    # cargo-fuzz (libFuzzer) targets for the vault
└── .github/workflows/ci.yml # fmt, clippy, tests, nightly long and libFuzzer runs
```

## What it does

You define the actions a user can take against your contract and the invariants that must hold. `soroban-fuzzer` then:

- generates random action sequences and inserts **ledger time jumps** between them,
- catches **missing `require_auth`**, **expired persistent/instance entries** (missing `extend_ttl`), **data loss in temporary storage**, **arithmetic panics**, **model mismatches** and **unbounded storage reads or CPU use** against a configurable share of mainnet limits,
- shrinks every failure to a minimal sequence and prints a seed that replays it,
- runs the same harness under **libFuzzer** for coverage-guided fuzzing.

See [`crates/soroban-fuzzer/README.md`](crates/soroban-fuzzer/README.md) for the full guide.

## Development

Prerequisites: Rust 1.91 or newer.

```bash
cargo test --workspace                         # unit, example and bug-detection tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
SOROBAN_FUZZ_CASES=2000 cargo test --workspace # a longer fuzz campaign
```

The two Wasm tests skip unless `examples/vault/fixtures/vault.wasm` holds a real build. `soroban-sdk` only builds contracts through [stellar-cli](https://github.com/stellar/stellar-cli):

```bash
cargo install --locked stellar-cli --no-default-features
stellar contract build --package vault-example
cp target/wasm32v1-none/release/vault_example.wasm examples/vault/fixtures/vault.wasm
```

Coverage-guided fuzzing needs nightly and cargo-fuzz:

```bash
cd fuzz
cargo +nightly fuzz run vault          # the correct vault: should run without crashing
cargo +nightly fuzz run vault_no_ttl   # a buggy vault: crashes within seconds
```
