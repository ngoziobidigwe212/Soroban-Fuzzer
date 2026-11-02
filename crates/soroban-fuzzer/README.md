# soroban-fuzzer

Stateful, property-based fuzzing for [Soroban](https://developers.stellar.org/docs/build/smart-contracts) smart contracts. It runs inside the native `soroban-sdk` test environment, so you write it as an ordinary `#[test]`.

You describe what users can do to your contract and what must always hold. The fuzzer generates random sequences of calls, inserts ledger time jumps between them, and checks each step for Soroban-specific bugs. When a check fails, it shrinks the sequence to a minimal reproduction:

```text
soroban-fuzzer found a failing case: ArchivedEntry
  hint: an entry expired: extend its TTL on access (or use a longer TTL)
  minimal sequence (2 steps):
      0: Deposit(ActorId(0), 1)
      1: <advance 4096 ledgers>  <-- failed here
  error: 1 entry expired at ledger 4096:
    Persistent entry of CAAA…D2KM key=Vec([Symbol(Balance), Address(…)]) (live until ledger 4095)
  replay: SOROBAN_FUZZ_SEED=1 cargo test
```

## What it catches

| Failure kind | Bug class | How it is detected |
|---|---|---|
| `MissingAuth` | missing `require_auth()` | Each address returned by `required_auths()` must appear in `env.auths()` after the call. |
| `ArchivedEntry` | missing or too-short `extend_ttl` | After every ledger move, the raw ledger is scanned for expired persistent or instance entries. |
| `InvariantViolated` | data loss, e.g. balances kept in `temporary()` storage | Your invariants are checked after every step, including time jumps. |
| `ModelMismatch` | logic bugs | `apply` compares the contract's result with your model. |
| `UnexpectedPanic` | overflow, `unwrap`, contract traps | Panics inside `apply` are caught and reported. |
| `ResourceBudgetExceeded` | unbounded loops, reads that grow with state | Per-call CPU, memory, footprint entries, bytes and events are compared with a budget (default 75% of mainnet limits). |
| `ResourceLimitExceeded` | calls that already fail on mainnet | The test `Env` enforces mainnet limits and the fuzzer classifies the resulting panic. |

> **Why scan for expiry instead of waiting for a failed read?** Since protocol 23, Soroban restores archived entries automatically when they are accessed. In the test environment, reading an expired entry therefore succeeds silently. Even the SDK's `get_ttl` restores it. The fuzzer reads the host's raw storage map instead, so it sees expiry without changing it. The [`ttl`](src/ttl.rs) helpers use the same approach.

## Quick start

```toml
[dev-dependencies]
soroban-fuzzer = { git = "https://github.com/soroban-dev-labs/Soroban-Fuzzer" }
soroban-sdk = { version = "29", features = ["testutils"] }
```

```rust
use std::collections::BTreeMap;
use soroban_fuzzer::prelude::*;

#[derive(Clone, Debug)]
enum Action {
    Deposit(ActorId, i128),
    Withdraw(ActorId, i128),
}

struct VaultFuzz {
    id: Address,
    actors: Actors,
    model: BTreeMap<ActorId, i128>,
}

impl Harness for VaultFuzz {
    type Action = Action;

    fn setup(env: &Env) -> Self {
        Self {
            id: env.register(Vault, ()),
            actors: Actors::generate(env, 4),
            model: BTreeMap::new(),
        }
    }

    fn actions() -> BoxedStrategy<Action> {
        prop_oneof![
            (actor(4), amount()).prop_map(|(a, x)| Action::Deposit(a, x)),
            (actor(4), amount()).prop_map(|(a, x)| Action::Withdraw(a, x)),
        ]
        .boxed()
    }

    fn apply(&mut self, env: &Env, action: &Action) -> Result<Outcome, String> {
        let client = VaultClient::new(env, &self.id);
        match *action {
            Action::Deposit(a, x) => {
                let who = self.actors.get(a);
                match client.try_deposit(who, &x) {
                    Ok(_) if x > 0 => { *self.model.entry(self.actors.id(a)).or_default() += x; Ok(Outcome::Executed) }
                    Err(Ok(Error::InvalidAmount)) if x <= 0 => Ok(Outcome::Rejected),
                    other => Err(format!("deposit({x}) returned {other:?}")),
                }
            }
            Action::Withdraw(a, x) => { /* … */ Ok(Outcome::Executed) }
        }
    }

    fn invariants(&self, env: &Env) -> Result<(), String> {
        let client = VaultClient::new(env, &self.id);
        for (id, addr) in self.actors.iter() {
            let want = self.model.get(&id).copied().unwrap_or(0);
            if client.balance(addr) != want {
                return Err(format!("actor {} balance drifted", id.0));
            }
        }
        Ok(())
    }

    fn required_auths(&self, _env: &Env, action: &Action) -> Vec<Address> {
        match *action {
            Action::Deposit(a, _) | Action::Withdraw(a, _) => vec![self.actors.get(a).clone()],
        }
    }
}

#[test]
fn fuzz_vault() {
    Fuzzer::<VaultFuzz>::new().cases(256).check();
}
```

For a complete harness, see [`tests/common/mod.rs`](tests/common/mod.rs). [`tests/bug_detection.rs`](tests/bug_detection.rs) has five buggy vaults, one per bug class, that the fuzzer must catch.

## Writing a harness

| Method | Required | Purpose |
|---|---|---|
| `setup(env)` | yes | Register contracts, generate actors and build the initial model. Runs once per case on a fresh `Env`. |
| `actions()` | yes | A proptest strategy for one action. Refer to accounts by `ActorId`, not `Address`. |
| `apply(env, action)` | yes | Call the contract and update the model. Return `Executed`, `Rejected` (an expected contract error) or `Skipped`. Return `Err` on a mismatch with the model. |
| `invariants(env)` | no | Properties that must hold after every step. |
| `required_auths(env, action)` | no | Who must sign. Make the state-changing call the last one in `apply`. |
| `watched_contracts()` | no | Restrict the expiry scan to your own contracts, e.g. to ignore a token contract the harness deployed. |

Tips:

- Use the `try_` client functions and match on the expected errors. A host-level failure (`Err(Err(_))`) means the contract trapped. Panic on it so it is reported as `UnexpectedPanic`.
- The strategies in `soroban_fuzzer::strategies` (`amount`, `amount_up_to`, `positive_amount`, `actor`, `ledger_jump`, `symbol_str`, `bytes`) are biased toward edge values such as `0`, `±1` and `i128::MIN/MAX`, but shrink toward small, readable numbers.
- To add a regression test for a failing case, copy its minimal sequence into `Fuzzer::replay(vec![Step::Call(..), Step::AdvanceLedger(..)])`.

## Configuration

```rust
Fuzzer::<MyHarness>::new()
    .cases(500)                                // sequences to run (default 256)
    .steps(1..=64)                             // sequence length (default 1..=32)
    .seed(42)                                  // fixed seed (default random)
    .ledger_jumps(LedgerJumps {
        weight: 2,                             // vs. 10 for an action
        max_jump: 50_000,                      // largest single jump
        horizon: 120_960,                      // state must stay live this long (≈7 days)
    })
    .resource_budget(ResourceBudget::mainnet_percent(60))
    .mock_all_auths(true)                      // default true
    .check_auths(true)                         // default true
    .check_ttl(true)                           // default true
    .check();                                  // panics with the report on failure
```

`run()` returns `Result<Report, Box<FuzzFailure<_>>>` if you want to inspect the result rather than panic.

`horizon` is the TTL your contract promises. A case never advances the ledger further than this, so a contract that extends entries to at least `horizon` ledgers on access passes. One that relies on the network minimum TTL fails.

Environment variables override the code settings:

| Variable | Effect |
|---|---|
| `SOROBAN_FUZZ_CASES` | Number of cases, e.g. a long nightly run in CI. |
| `SOROBAN_FUZZ_SEED` | Replays a failure with the seed printed in its report. |

## Limitations

- Resource checks use the metering of the **last** contract call in each `apply`. If one action makes several calls, only the last is checked.
- Natively registered contracts skip Wasm VM costs. To measure those, register the compiled Wasm with `env.register(WASM, ())`.
- Resource limits are the mainnet snapshot shipped with `soroban-sdk` 29.
