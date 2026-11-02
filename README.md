# Soroban-Fuzzer

[![CI](https://github.com/soroban-dev-labs/Soroban-Fuzzer/actions/workflows/ci.yml/badge.svg)](https://github.com/soroban-dev-labs/Soroban-Fuzzer/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/soroban-fuzzer.svg)](https://crates.io/crates/soroban-fuzzer)
[![docs.rs](https://img.shields.io/docsrs/soroban-fuzzer)](https://docs.rs/soroban-fuzzer)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Stateful fuzzing for Soroban smart contracts, running in the `soroban-sdk` test environment. It finds missing `require_auth`, auth that isn't enforced, storage that expires, data lost from temporary storage, arithmetic traps and calls that blow mainnet resource limits, and shrinks each failure to a few steps you can replay.

## Part of soroban-guard

This repository is one part of the **soroban-guard** safety suite for Soroban:

| Repository | What it does |
|---|---|
| [Soroban-Storage](https://github.com/soroban-dev-labs/Soroban-Storage) | Typed storage wrappers that make lifecycle and TTL rules explicit. |
| [Soroban-Linter](https://github.com/soroban-dev-labs/Soroban-Linter) | Static analysis: detectors for auth, storage lifecycle, arithmetic and resource limits. |
| **Soroban-Fuzzer** | Runtime checks: drives real contracts through random call sequences and ledger time to find the same bug classes in running code. |

The linter flags risky patterns in source code. The fuzzer shows whether they turn into real bugs, and catches the ones static analysis can't see.

## What makes it different

- **Tested on production code.** Besides its own examples, it fuzzes the [Stellar Asset Contract](examples/stellar-asset), the built-in token behind every Stellar asset, against a model of its rules: transfers, expiring allowances, frozen holders and clawback.
- **Its detection is tested.** Five deliberately buggy vaults, one per bug class, must each be caught on three seeds and shrunk to a short sequence (two or three steps for most). CI fails if any check stops working.
- **Soroban-aware storage checks.** Since protocol 23, reading an expired entry silently restores it in tests, which hides TTL bugs. The fuzzer reads the host's raw storage instead, so expiry is seen without side effects.
- **Auth is checked two ways.** It checks that every required signer actually signed, and it replays each signed action with no signatures to prove the contract really enforces auth.
- **Two modes, one harness.** Property-based runs in `cargo test` on stable Rust, and coverage-guided runs under cargo-fuzz and libFuzzer.

```
.
├── crates/
│   └── soroban-fuzzer/      # the fuzzing library (see its README)
├── examples/
│   ├── vault/               # token vault, its harness, 5 buggy variants and the tests
│   └── stellar-asset/       # harness for the built-in Stellar Asset Contract
├── fuzz/                    # cargo-fuzz (libFuzzer) targets for both examples
└── .github/workflows/ci.yml # fmt, clippy, tests, nightly long and libFuzzer runs
```

## What it does

You define the actions a user can take against your contract and the invariants that must hold. `soroban-fuzzer` then:

- generates random action sequences and inserts **ledger time jumps** between them,
- catches **missing `require_auth`**, **expired persistent/instance entries** (missing `extend_ttl`), **data loss in temporary storage**, **arithmetic panics**, **model mismatches** and **unbounded storage reads or CPU use** against a configurable share of mainnet limits,
- shrinks every failure to a minimal sequence and prints a seed that replays it,
- runs the same harness under **libFuzzer** for coverage-guided fuzzing.

See [`crates/soroban-fuzzer/README.md`](crates/soroban-fuzzer/README.md) for the full guide.

## Contributing

Issues labelled [`good first issue`](https://github.com/soroban-dev-labs/Soroban-Fuzzer/labels/good%20first%20issue) are scoped for newcomers, and every issue states its difficulty and what "done" means. See [CONTRIBUTING.md](CONTRIBUTING.md). Report security problems privately, as described in [SECURITY.md](SECURITY.md).

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
FLAGS="-max_len=256 -len_control=0"
cargo +nightly fuzz run -s none vault         -- $FLAGS  # correct vault: runs without crashing
cargo +nightly fuzz run -s none stellar_asset -- $FLAGS  # the Stellar Asset Contract: same
cargo +nightly fuzz run -s none vault_no_ttl  -- $FLAGS  # buggy vault: crashes within seconds
```
