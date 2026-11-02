# Contributing

Thanks for helping make Soroban contracts safer. Issues labelled
`good first issue` or `help wanted` are scoped so they can be picked up
independently. Each issue states its difficulty and what "done" means.
Comment on an issue to claim it before starting.

## Setup

1. Install Rust (stable, 1.91 or newer).
2. For the Wasm tests, install [stellar-cli](https://github.com/stellar/stellar-cli)
   and build the fixture (see the README's *Development* section). Without it
   those two tests skip; everything else runs.
3. For coverage-guided fuzzing, install nightly and
   [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz).

## Before opening a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## Guidelines

- **Every behaviour change needs a test.** A new check needs both sides: a
  correct contract that passes it, and a buggy one that fails it with the
  expected `FailureKind` and a short shrunk sequence (see
  `examples/vault/src/test/bugs.rs`).
- **No false alarms.** The correct vault and the Stellar Asset Contract
  harness must keep passing with every check on. If you change a check, run
  them with more cases: `SOROBAN_FUZZ_CASES=2000 cargo test --workspace`.
- **Keep runs reproducible.** Anything random must come from the proptest
  runner, so a seed always replays the same case.
- **Model real rules exactly.** When a harness models an existing contract
  (like the SAC), follow its real check order and error codes, and link the
  source you used.
- **If you touch `fuzz_one` or the step strategy,** run each correct cargo-fuzz
  target for a minute and the `vault_no_ttl` target until it crashes.
- Public items need doc comments; `cargo doc` must build without warnings.
- Add a line to `crates/soroban-fuzzer/CHANGELOG.md` under "Unreleased".
