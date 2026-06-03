# Replay, Spectator, and Tournament System

This document describes how Magnetite records a complete deterministic log of every
match, how that log is stored and served, how the anti-cheat service verifies it
for tamper-evidence, and how it is played back inside a browser using the
`magnetite-web-client`. It also covers the tournament system exposed at
`/api/v1/tournaments`.

---

## 1. ReplayLog — recording a match

### 1.1 Data structure

`ReplayLog` is defined in `backend/magnetite-sdk/src/authority.rs` (line 917):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    /// The match configuration used during the original run.
    pub config: MatchConfig,

    /// Per-tick input records: (tick, [(player_id, input)]).
    pub frames: Vec<(Tick, Vec<(PlayerId, Input)>)>,

    /// Per-tick state hashes recorded by the runtime: (tick, hash).
    pub state_hashes: Vec<(Tick, u64)>,
}
```

`config` carries the full `MatchConfig` (topology, tick rate, seed, snapshot cadence)
so the log is self-contained — re-simulation never needs external context.

`frames` is the ordered sequence of sanitised inputs that the executor actually
processed. Anticheat-rejected inputs are never recorded here: only the inputs that
passed the `ValidatorChain` and reached `GameExecutor::step` appear in this list.

`state_hashes` contains one FNV-1a 64-bit hash per tick, computed over the
canonical JSON serialisation of the game `Snapshot`. FNV-1a is used instead of
`DefaultHasher` because `DefaultHasher` is SipHash with a random per-process seed,
which would make hashes non-reproducible across server restarts.

### 1.2 How the runtime records it

`TickScheduler` (in `magnetite-runtime/src/tick.rs`) owns a
`Arc<Mutex<ReplayLog>>` initialised at construction time:

```rust
pub struct TickScheduler {
    executor:      Arc<Mutex<Box<dyn GameExecutor>>>,
    connection_mgr: ConnectionManager,
    config:        MatchConfig,
    replay_log:    Arc<Mutex<ReplayLog>>,   // ← the recording
    anticheat:     Arc<Mutex<Anticheat>>,
}
```

Inside `run_tick`, after the anticheat pass and the executor step, the sanitised
input list and the resulting `state_hash` are committed atomically:

```rust
// record sanitised inputs only, matching what the executor actually processed
{
    let mut log = self.replay_log.lock().await;
    log.record(tick, sanitised_inputs.clone(), step_out.state_hash);
}
```

`ReplayLog::record` is a two-line append:

```rust
pub fn record(&mut self, tick: Tick, inputs: Vec<(PlayerId, Input)>, state_hash: u64) {
    self.frames.push((tick, inputs));
    self.state_hashes.push((tick, state_hash));
}
```

The anticheat and game-session end handlers can retrieve the log via:

```rust
pub fn replay_log(&self) -> Arc<Mutex<ReplayLog>> {
    Arc::clone(&self.replay_log)
}
```

### 1.3 Storage

`ReplayLog` implements `serde::Serialize` / `Deserialize`. After a match ends the
caller serialises the log to JSON (or MessagePack) and persists it to the database
or an object store. The `ws/game.rs` handler calls `store_replay()` on session
end — this is the integration point between the runtime and the platform storage
layer.

---

## 2. verify_replay — tamper-evidence

### 2.1 Core algorithm

`verify_replay<G: AuthoritativeGame>` in `authority.rs` (line 1029) performs a
complete re-simulation:

```rust
pub fn verify_replay<G: AuthoritativeGame>(log: &ReplayLog) -> ReplayVerdict {
    // Re-create the executor from the same config
    let mut exec = NativeExecutor::<G>::new(log.config.clone());

    // Build a tick → expected_hash lookup
    let hash_map: HashMap<Tick, u64> = log.state_hashes.iter().copied().collect();

    for (tick, inputs) in &log.frames {
        let out = exec.step(*tick, inputs);

        if let Some(&expected) = hash_map.get(tick) {
            if out.state_hash != expected {
                return ReplayVerdict::Divergence { tick: *tick, expected, got: out.state_hash };
            }
        }
    }

    ReplayVerdict::Clean
}
```

Steps:
1. Reconstruct a fresh `NativeExecutor` from `log.config` — same seed, same tick
   rate, same topology.
2. Feed every recorded input frame into `executor.step` in the original order.
3. After each tick compare the re-simulated `state_hash` with the recorded hash.
4. First mismatch → `ReplayVerdict::Divergence { tick, expected, got }`.
5. No mismatch after all ticks → `ReplayVerdict::Clean`.

### 2.2 ReplayVerifier — enriched diagnostics

`magnetite-anticheat/src/replay_verifier.rs` wraps `verify_replay` with a richer
result type:

```rust
pub enum VerificationResult {
    Clean,
    Divergence {
        tick:              u64,
        expected:          u64,
        got:               u64,
        suspected_players: Vec<PlayerId>,   // heuristic
    },
}
```

`suspected_players` is the set of players who sent an input at the diverging tick.
This is a heuristic — nondeterminism bugs in game code can also produce divergence
— but it provides a starting point for investigation.

`ReplayVerifier` is stateless; a single instance can verify many logs:

```rust
let verifier = ReplayVerifier::new();
let result = verifier.verify::<MyGame>(&log);
```

### 2.3 What divergence means

| Cause | Result |
|---|---|
| Tampered input bytes in the stored log | `Divergence` — re-simulation produces different state |
| Tampered state hash in the stored log | `Divergence` — recorded hash no longer matches re-simulation |
| Injected extra input entries | `Divergence` — extra commands alter state |
| Nondeterminism bug in game code (wall clock, OS RNG, etc.) | `Divergence` on the first affected tick |
| Honest match, deterministic game | `Clean` |

The `magnetite-e2e/tests/convergence.rs` suite asserts that the reference
`ArenaShooter` game always returns `Clean` after a full multi-tick run.

---

## 3. Serving replays

Replays are opaque JSON blobs stored after a match ends. The serving path is:

1. A client requests a replay for a completed match via the REST API. The backend
   reads the serialised `ReplayLog` from the database.
2. The backend returns the raw JSON. No special transformation is needed: the log
   is self-describing (contains `config`, all `frames`, and all `state_hashes`).
3. The client — either the `magnetite-web-client` or a native Bevy client — parses
   the log and begins playback.

### 3.1 Spectator stream

The live spectator path does not use `ReplayLog` directly. A spectator connects
to the game WebSocket with a read-only token. The runtime's per-tick fan-out loop
(step 5 of `run_tick`) sends `ServerNet::Snapshot` and `ServerNet::Delta` frames
to every registered connection, including spectators. The spectator receives the
same interest-filtered deltas as players, but never sends `ClientNet::InputFrame`
messages.

---

## 4. In-browser playback — magnetite-web-client

`magnetite-web-client/src/` is a zero-dependency vanilla-JS client that speaks the
`ClientNet` / `ServerNet` protocol exactly as defined in `magnetite-sdk::protocol`.

### 4.1 Wire protocol (browser side)

`src/protocol.js` documents the exact wire shape:

| Message | Direction | Fields |
|---|---|---|
| `input_frame` | client → server | `{ seq: u32, tick: u64, input }` |
| `welcome` | server → client | `{ player_id, config }` |
| `snapshot` | server → client | `{ tick, full }` — `full` is a base64-encoded JSON snapshot |
| `delta` | server → client | `{ tick, since_tick, diff }` — `diff` is base64-encoded JSON delta |
| `ack` | server → client | `{ seq, tick }` |
| `reject` | server → client | `{ seq, reason }` |

`Vec<u8>` fields (`full`, `diff`) are serialised by `serde_json` as base64 strings.
`decodeBytes()` in `protocol.js` handles both base64 strings and raw byte arrays.

### 4.2 Entry point

```js
import { createClient } from './magnetite-web-client/src/client.js';

const client = createClient({
  url:    'ws://localhost:9001',
  token:  'jwt-from-platform',
  canvas: document.getElementById('game'),
});
client.connect();
client.onState(state => { /* state is ArenaView */ });
```

`createClient` returns a `MagnetiteClient` that:

- Opens a WebSocket via `ConnectionManager` (with exponential-backoff reconnect).
- Waits for `ServerNet::Welcome` to get `player_id` and `MatchConfig`, then starts
  a `setInterval` tick loop at `config.tick_hz` Hz and a `requestAnimationFrame`
  render loop.
- Captures keyboard + mouse input via `InputCapture` and sends one `input_frame`
  per tick.
- Applies inputs locally through `PredictionBuffer` for immediate visual feedback
  (client-side prediction).
- Reconciles against authoritative `Snapshot` and `Delta` frames from the server.

### 4.3 Client-side prediction (PredictionBuffer)

`src/prediction.js` implements the standard prediction-reconciliation loop:

- On each tick, `predict(seq, tick, input)` applies the input to the current
  predicted state and buffers the `(seq, tick, input)` triple.
- On `ServerNet::Ack { seq }`, all buffered frames with `seq ≤ acked` are discarded.
- On `ServerNet::Snapshot { tick, full }`, the buffer is pruned to frames after
  `tick`, then all remaining buffered frames are replayed on top of the authoritative
  snapshot. This is the "rollback and replay" reconciliation step.
- On `ServerNet::Reject { seq }`, the rejected frame is removed and the predicted
  state is re-derived from the last authoritative snapshot.

### 4.4 Canvas renderer

`src/renderer.js` provides a default arena-shooter renderer. The `render` option
to `createClient` is pluggable — any function `(CanvasRenderingContext2D, state, playerId) => void`
can be substituted for games with different view shapes.

The renderer uses a 200 × 200 world-unit coordinate system (matching
`game-template-authoritative`), centred in the canvas. It draws:

- Arena floor and grid lines.
- Projectiles (own = teal, enemy = orange).
- Enemy players (red circles with barrel direction indicator and HP bar).
- Local player (white outline, teal fill).
- HUD overlay (tick counter, HP, score, player count).

### 4.5 Replay playback in-browser

To play back a stored `ReplayLog` in the browser, step through `log.frames` at the
original `config.tick_hz` rate, feeding each tick's inputs into the same
`applyDeltaToSnapshot` / `snapshotToView` pipeline the live client uses. Because
the log contains the canonical inputs and all state hashes were produced by
deterministic game code, the browser can re-simulate the match frame-by-frame
without a live server connection.

A minimal replay driver:

```js
import { createClient } from './magnetite-web-client/src/client.js';
import { applyDeltaToSnapshot, snapshotToView } from './magnetite-web-client/src/delta.js';

// `log` is the parsed ReplayLog JSON from the REST API.
// `canvas` is an HTMLCanvasElement.
async function playReplay(log, canvas) {
  const ctx = canvas.getContext('2d');
  const tickMs = 1000 / log.config.tick_hz;
  let snapshot = null;

  for (const [tick, inputs] of log.frames) {
    // (In a real player: advance a local game state here using the recorded inputs.
    //  For the arena-shooter, apply arenaApplyInput for each input entry.)

    // Then render whatever state you have derived:
    // renderArenaView(ctx, view, null);  // null = no local player (spectator)

    await new Promise(r => setTimeout(r, tickMs));
  }
}
```

For a fully deterministic replay the browser would need to host a WASM build of
the same game (compiled with `--features wasm`), calling the same `mag_step` ABI
the server does. The JS `arenaApplyInput` approximation is suitable for spectators
and highlights reels but may diverge from the authoritative state over long
sequences.

---

## 5. Tournament system

`backend/src/api/tournaments.rs` is mounted at `/api/v1/tournaments` and exposes
bracket management, registration, and match-result submission.

### 5.1 Data model

| Type | Key fields |
|---|---|
| `Tournament` | `id`, `name`, `game_id`, `status`, `max_players`, `entry_fee`, `prize_pool`, `start_time` |
| `TournamentParticipant` | `id`, `tournament_id`, `user_id`, `status`, `seed`, `registered_at` |
| `TournamentMatch` | `id`, `tournament_id`, `round`, `match_number`, `player1_id`, `player2_id`, `winner_id`, `player1_score`, `player2_score`, `status`, `scheduled_at`, `completed_at` |

`TournamentStatus` is a PascalCase string enum: `Draft`, `Registration`,
`InProgress`, `Completed`, `Cancelled`.

### 5.2 Lifecycle and bracket generation

1. **Create** — `POST /api/v1/tournaments` (auth required). Status starts at
   `Draft`. Validates that the referenced `game_id` exists and is active.

2. **Update** — `PUT /api/v1/tournaments/:id` (auth required). Edits name, status,
   player cap, entry fee, prize pool, or start time. Only `Draft` and `Registration`
   statuses are mutable.

3. **Register** — `POST /api/v1/tournaments/:id/register` (auth required). The
   tournament must be in `Registration` status and not yet full (checks
   `COUNT(*) ... WHERE status = 'registered'` against `max_players`). Upserts the
   participant row so re-registering is idempotent.

4. **Start** — `POST /api/v1/tournaments/:id/start` (auth required). Requires
   `Registration` status and at least 2 participants. Generates a full single-elimination
   bracket:

   - Computes `num_rounds = ceil(log2(num_players))`.
   - For each round `r` from 1 to `num_rounds`, creates `2^(num_rounds − r)` match
     rows with `status = 'pending'`.
   - Transitions the tournament to `InProgress`.

   Match rows are seeded by `ORDER BY seed NULLS LAST, registered_at`. Player
   assignment to individual match slots is left to subsequent operations (round 1
   players are not yet written into `player1_id` / `player2_id` by the start
   handler — the bracket skeleton is created and the platform or an admin fills
   slots via updates).

5. **Submit result** — `POST /api/v1/tournaments/:id/match/:match_id/result` (auth
   required). One of the two match participants submits `winner_id`,
   `player1_score`, `player2_score`. The handler enforces that:
   - The tournament is `InProgress`.
   - The match exists and belongs to the tournament.
   - The match is not already `completed`.
   - The caller is `player1_id` or `player2_id`.

### 5.3 REST endpoints

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/api/v1/tournaments` | Public | List tournaments. Query: `status`, `game_id`, `page`, `per_page`. Returns paginated. |
| `POST` | `/api/v1/tournaments` | Required | Create a tournament. |
| `GET` | `/api/v1/tournaments/:id` | Public | Tournament details including participants and all matches. |
| `PUT` | `/api/v1/tournaments/:id` | Required | Update a tournament (Draft/Registration only). |
| `POST` | `/api/v1/tournaments/:id/register` | Required | Register the current user. |
| `POST` | `/api/v1/tournaments/:id/start` | Required | Start the tournament and generate the bracket. |
| `POST` | `/api/v1/tournaments/:id/match/:match_id/result` | Required | Submit match result. |

### 5.4 Connection to ReplayLog

The tournament system does not yet directly link match rows to `ReplayLog` storage
— that association (e.g. `replay_id` foreign key on `tournament_matches`) is a
planned future addition. The practical wiring is:

- When a `ws/game.rs` session ends and `store_replay()` is called, the resulting
  replay identifier can be written back to the `tournament_matches` row for that
  match.
- Spectators and post-match viewers can then fetch the replay blob via the replay
  REST endpoint and play it back in-browser using the `magnetite-web-client`.

---

## 6. Honest gaps

| Item | Status | Notes |
|---|---|---|
| Tournament match slot assignment | Partial | `start_tournament` creates bracket skeleton rows but does not assign `player1_id` / `player2_id` for round 1 based on participant seeds. A subsequent admin action or a future handler must fill these slots. |
| Replay storage REST API | Not yet wired | `store_replay()` in `ws/game.rs` persists the blob; no GET endpoint for retrieving a replay by match ID exists yet. |
| Tournament-to-replay link | Not yet wired | No `replay_id` foreign key on `tournament_matches`. |
| Live spectator auth | Same as other WS paths | Spectator connections use the same `?token=JWT` query-param pattern as `ws/game`, `ws/comms`, and `ws/notifications`. |
| Prize distribution | Not implemented | `prize_pool` is stored but no payment disbursement logic exists post-completion. |
