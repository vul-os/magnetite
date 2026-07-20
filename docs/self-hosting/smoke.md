# Smoke Test

`scripts/smoke.sh` is the Magnetite local-stack smoke test.  It exercises as
much of the stack as possible without external credentials or a running cloud
environment, and prints a clear `PASS / FAIL / SKIP` result per step plus an
overall result.

## Quick start

```bash
bash scripts/smoke.sh
```

Results are written to `/tmp/magnetite-smoke.txt` for auditing and also
printed to stdout with colour highlights.

## Steps

| Step | What it proves | Prereqs | Skip condition |
|------|---------------|---------|----------------|
| 1 | `docker-compose.yml` parses; postgres, redis, backend, frontend services all defined (plus `mediamtx`, defined but behind the optional `media` profile) | `docker` on PATH | `docker` / `docker compose` absent |
| 2 | `game-template-authoritative` compiles to `wasm32-wasip1` with the `mag_*` ABI | `cargo`, `rustup`, Rust nightly wasm32-wasip1 target | `SKIP_WASM_BUILD=1`, or `cargo`/`rustup` absent, or target unavailable |
| 3 | Full-stack WebSocket tests: real GameServer + tokio-tungstenite clients — Welcome, Snapshot, Delta, Ack, convergence, replay | `cargo` | `cargo` absent |
| 4 | `verify_replay` returns `Clean` over 20 ticks (in-proc determinism proof) | `cargo` | `cargo` absent |
| 5 | `WasmExecutor` produces identical `state_hash` to `NativeExecutor` on every tick (sandbox parity) | wasm artifact from Step 2 | Step 2 was skipped or failed |

A **SKIP** is not a failure. The overall result is `PASS` if no step fails.

## Environment variables

| Variable | Default | Effect |
|----------|---------|--------|
| `SKIP_WASM_BUILD` | `0` | Set to `1` to skip the wasm build (Step 2). If a prior artifact exists it is still used for Step 5. |
| `CARGO_VERBOSE` | `0` | Set to `1` to pass `-v` to `cargo build` in Step 2. |

## Prerequisites

**Minimum (Steps 3 + 4 only):**

```bash
cargo   # Rust stable toolchain
```

**Full run (all steps):**

```bash
docker          # Docker Desktop or Engine with Compose v2
cargo           # Rust stable toolchain
rustup          # to install wasm32-wasip1 target if absent
```

Install the wasm target once:

```bash
rustup target add wasm32-wasip1
```

## Running in CI

The smoke script is safe to run in CI pipelines that have a Rust toolchain
available.  Steps that require Docker are automatically skipped when `docker`
is not on the PATH.

```yaml
# Example GitHub Actions step
- name: Smoke test
  run: bash scripts/smoke.sh
  env:
    SKIP_WASM_BUILD: "1"   # omit if the CI runner has wasm32-wasip1
```

## Manual step-by-step

If the script fails at a specific step, reproduce it manually:

### Step 1 — docker-compose config

```bash
docker compose -f docker-compose.yml config --quiet
docker compose -f docker-compose.yml config --services
```

### Step 2 — wasm build

```bash
cd game-templates/authoritative
cargo build --release --target wasm32-wasip1 --features wasm
# artifact: target/wasm32-wasip1/release/game_template_authoritative.wasm
```

See also `scripts/wasm-build-runner.sh` for the full build-runner pipeline
(daemon mode, S3 upload, CI integration).

### Steps 3 + 4 — runtime / replay tests

```bash
cd magnetite-e2e
cargo test --test fullstack_ws   -- --nocapture
cargo test --test convergence    -- --nocapture
```

### Step 5 — wasm parity

```bash
cd magnetite-e2e
cargo test --test wasm_end_to_end -- --nocapture
```

Or run the whole MOAT demo pipeline in one command:

```bash
bash scripts/moat-demo.sh
```

## What the runtime tests assert

**Step 3 (`fullstack_ws`)** starts a real `GameServer` on an ephemeral TCP
port with `NativeExecutor<ArenaShooter>` and connects three independent
`tokio-tungstenite` clients.  It asserts:

- Every client receives a `Welcome` frame immediately on connect.
- Every client receives at least one `Snapshot` (server-authoritative
  full-state broadcast).
- At least one client receives at least one `Delta` (tick-level diff).
- The server produces input responses (`Ack` or `Reject`) for every
  `InputFrame` sent.
- Two independent `NativeExecutor` runs over the same inputs produce
  identical `state_hash` on every tick (cross-run convergence).
- `verify_replay` returns `ReplayVerdict::Clean` over the recorded
  `ReplayLog` (tamper-evident replay verification passes).
- Snapshot tick values are non-decreasing (tick counter never regresses).
- The `Dedicated` topology also starts and broadcasts state correctly.

**Step 4 (`convergence`)** runs the in-proc replay proof independently,
confirming the above without WebSocket overhead.

**Step 5 (`wasm_end_to_end`)** loads the compiled `game_template_authoritative.wasm`
via `WasmExecutor` (Wasmtime, fuel-metered, epoch-bounded) and asserts that
every tick's `state_hash` is identical to the `NativeExecutor` run —
the core MOAT sandbox-determinism guarantee.

## Troubleshooting

**`wasm artifact missing` panic in Step 5**
Run Step 2 first (or `bash scripts/moat-demo.sh`).

**`docker compose config` fails**
Check that `docker-compose.yml` is valid YAML and that any `${VAR}` values
that are required by the compose validator are set in `.env`.

**Cargo lock conflicts during parallel agents**
Run the smoke script after other `cargo` processes have finished; the Cargo
file-lock resolves automatically once the build cache is warm.

**All tests pass but Step 2 is SKIP**
That is expected on machines without the `wasm32-wasip1` target.  Run
`rustup target add wasm32-wasip1` once to enable Step 2 and Step 5.
