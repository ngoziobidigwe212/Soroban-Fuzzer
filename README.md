# Soroban-Fuzzer

Property-based fuzzing harness for Soroban smart contracts, running in the `soroban-sdk` test environment.

This repository holds the fuzzing part of the **soroban-guard** safety suite. The other planned parts are typed storage wrappers, a security linter and a rent-keeper daemon.

```
.
├── crates/
│   └── soroban-fuzzer/      # the fuzzing harness (see its README)
│       ├── src/
│       └── tests/           # a correct vault plus 5 buggy vaults it must catch
└── .github/workflows/ci.yml # fmt, clippy, tests, longer fuzz run
```

## What it does

You define the actions a user can take against your contract and the invariants that must hold. `soroban-fuzzer` then:

- generates random action sequences and inserts **ledger time jumps** between them,
- catches **missing `require_auth`**, **expired persistent/instance entries** (missing `extend_ttl`), **data loss in temporary storage**, **arithmetic panics**, **model mismatches** and **unbounded storage reads or CPU use** against a configurable share of mainnet limits,
- shrinks every failure to a minimal sequence and prints a seed that replays it.

See [`crates/soroban-fuzzer/README.md`](crates/soroban-fuzzer/README.md) for the full guide.

## Development

Prerequisites: Rust 1.91 or newer.

```bash
cargo test --workspace                         # unit + bug-detection tests
cargo clippy --workspace --all-targets -- -D warnings
SOROBAN_FUZZ_CASES=2000 cargo test --workspace # a longer fuzz campaign
```
