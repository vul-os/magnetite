# Developer Quickstart

Get from zero to a published Rust game on Magnetite in five steps.

There are two paths to the same destination:

| Path | Tools needed | When to use |
|------|-------------|-------------|
| **Web Studio** (in-browser) | A modern browser | Zero local setup; best for exploring the platform and rapid prototyping |
| **CLI** (local) | Rust + `magnetite` CLI | Full control; required for advanced games and CI/CD |

The two paths share the same runtime (`magnetite-runtime`), the same
SDK (`magnetite-sdk::authority`), and the same protocol
(`ServerNet`/`ClientNet`). See the
[develop-in-browser guide](../moat/develop-in-browser.md) for the full
Web Studio walkthrough. This page covers the CLI path and notes where
the Studio diverges.

---

## CLI prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `wasm32-wasip1` target | — | `rustup target add wasm32-wasip1` |
| `magnetite` CLI | — | `cargo install --path magnetite-cli` |
| Docker + Compose | 24+ / 2.20+ | https://docs.docker.com (for the local platform stack) |

> **Web Studio path:** no local tools required — sign in to the Studio on
> whichever Magnetite storefront you use, pick a template, and the scaffold is
> generated for you in the browser. Skip to [Step 2](#step-2--implement-gamelog).
>
> `magnetite.gg` and `api.magnetite.gg` appear throughout these docs as
> **placeholder hostnames**. Magnetite operates no central cloud; substitute
> the node or storefront you are actually talking to.

---

## Step 1 — Scaffold a new game crate

### Option A — CLI

```bash
magnetite new my-game
cd my-game
```

> `magnetite new` takes a name only — there is **no `--template` flag**. It
> emits one canonical scaffold. To start from a richer starter, copy
> `game-template/`, `game-template-authoritative/`, `game-template-fps/`, or
> `game-template-motorsport/` from this repository instead.

`magnetite new` creates a ready-to-build crate:

```
my-game/
├── Cargo.toml    # cdylib + rlib; [features] wasm = []
└── src/
    └── lib.rs    # AuthoritativeGame stub + Wasm ABI exports (mag_init, mag_step, …)
```

Template tiers (`minimal`, `arena-shooter`, `fps-starter`,
`motorsport-starter`) are offered by the **Web Studio**, which scaffolds from
`src/data/templates.js` — not by the CLI. See the
[in-browser guide](../moat/develop-in-browser.md#step-1--create-a-project-in-the-web-studio)
for per-tier details.

### Option B — Web Studio (no local Rust needed)

Sign in at **Developer Portal → Studio → New Game** → pick a template
tier. The Studio generates the same scaffold, pre-connects a GitHub repo,
and lets you trigger builds from the UI. When you are ready to run
locally, clone the repo and follow the CLI steps below.

---

## Step 2 — Implement `AuthoritativeGame`

Open `src/lib.rs`. Replace the stub types with your real game data, then
fill in the five required methods of the `AuthoritativeGame` trait from
`magnetite_sdk::authority`.

The trait is documented fully in the
[moat quickstart](../moat/quickstart.md#step-2--implement-authoritativegame).
The short version:

```rust
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

pub struct MyGame { /* your state here */ }

impl AuthoritativeGame for MyGame {
    type Snapshot = MySnapshot;   // full state — Serialize + DeserializeOwned + Clone
    type Delta    = MyDelta;      // compact diff — Serialize + DeserializeOwned
    type View     = MyView;       // per-player filtered view — Serialize only
    type Command  = MyCommand;    // validated command — Serialize + DeserializeOwned

    fn init(cfg: &MatchConfig) -> Self { /* create fresh match state */ }
    fn validate(&self, player: PlayerId, input: &Input, tick: Tick)
        -> Result<Vec<MyCommand>, RejectReason> { /* translate raw input → commands */ }
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, MyCommand)]) { /* tick */ }
    fn snapshot(&self) -> MySnapshot { /* full state */ }
    fn restore(snap: &MySnapshot, cfg: &MatchConfig) -> Self { /* reconstruct */ }
    fn delta(&self, since: &MySnapshot) -> MyDelta { /* compact diff */ }
    fn view_for(&self, player: PlayerId) -> MyView { /* interest-filtered */ }
}
```

**Determinism contract (enforced by the replay verifier):**

- Use **only** `ctx.rng` (`DeterministicRng` / xoshiro256\*\*) for randomness.
- Never read wall-clock time in `step` or `validate`.
- Prefer `f32` with bounded per-tick deltas over `f64` accumulation.
- No I/O, threads, or blocking in `step` / `validate` (the Wasmtime sandbox
  strips these capabilities).

See the [SDK Reference](./sdk.md) for all types, and the reference
implementation in `game-template-authoritative/src/game.rs`.

---

## Step 3 — Build to Wasm

```bash
magnetite build
```

Runs `cargo build --release --target wasm32-wasip1 --features wasm` and
prints the artifact path:

```
Building `my-game` for wasm32-wasip1…
Build succeeded.
Artifact: target/wasm32-wasip1/release/my_game.wasm
```

> **Web Studio / CI path:** push to your connected repo. The Magnetite
> GitHub App triggers the same build and streams logs to
> **Studio → Builds**.

---

## Step 4 — Run locally and play in the browser

```bash
magnetite dev
```

One command: builds the artifact, loads it into `magnetite-sandbox`
(Wasmtime, fuel-metered, memory-capped), starts `magnetite-runtime` in
`SingleRoom` topology, and prints a WebSocket URL plus a **Play URL**:

```
  Connect URL : ws://127.0.0.1:54321
  Play URL    : http://localhost:54321/play?token=dev-token
  Topology    : SingleRoom (max 4 players)
  Tick rate   : 60 Hz
```

Open the Play URL in any modern browser. `magnetite-web-client` — the
lightweight in-browser canvas client — connects over WebSocket, receives
`ServerNet::Welcome` + `Snapshot`, and begins the tick loop. Keyboard,
mouse, and gamepad inputs are captured automatically.

> **Studio Preview path:** after a successful CI build click **Preview**
> in the Studio. The platform provisions a `magnetite-runtime` instance
> and opens the same `magnetite-web-client` play view pointed at the
> provisioned URL. No local server needed.

For full details on the browser client and the `ServerNet`/`ClientNet`
protocol, see the
[develop-in-browser guide](../moat/develop-in-browser.md).

---

## Step 5 — Connect your repository

Point Magnetite at your source so the CI pipeline can build and verify your game.

**Via the Studio / dashboard** (recommended):

1. Sign in → Developer Portal → **Studio → Connect Repository**.
2. Install the Magnetite GitHub App on your repo.
3. The platform records the installation and monitors push events.
4. After the next push, build logs appear in **Studio → Builds**.

**Via the API** (for automation):

```bash
# 1. Authenticate
TOKEN=$(curl -s -X POST https://api.magnetite.gg/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"you","password":"…"}' | jq -r .data.access_token)

# 2. Register the repo
curl -X POST https://api.magnetite.gg/api/v1/github/repos/register \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"your-github-user","repo":"my-game","installation_id":"<id>"}'
```

After registration the GitHub webhook at `/api/v1/github/webhooks/github` receives push events and
records build status. Poll build status with:

```bash
GET /api/v1/github/repos/{owner}/{repo}/build-status
```

> **Web Studio path:** repo connection happens during project creation (Step 1). No separate
> registration step required.

---

## Step 6 — Publish your game

Once CI is green (and you have previewed the game in-browser via Studio or `magnetite dev`),
publish through the Developer Portal or the CLI:

### Option A — CLI

```bash
export MAGNETITE_API_URL=https://api.magnetite.gg
export MAGNETITE_GAME_ID=<uuid-from-studio>
export MAGNETITE_API_TOKEN=<your-bearer-token>

magnetite deploy
```

`magnetite deploy` builds a fresh artifact, registers it with the
distribution API, and prints the version ID + promotion instructions.

### Option B — Studio

Click **Publish** in **Studio → Versions**. The Studio calls the same
distribution API and opens the game in the marketplace once admin
approval is complete.

### Option C — API

```bash
# 1. Register the game record (first time only)
curl -X POST https://api.magnetite.gg/api/v1/games \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "My Game",
    "description": "…",
    "max_players": 4,
    "tick_rate": 60,
    "wasm_artifact_url": "https://…/my_game.wasm"
  }'

# 2. Submit for review (admin approves before going live)
# PUT /api/v1/developer/games/{id}/status
# { "status": "pending_review" }
```

After admin approval the game appears in the marketplace and players can
discover, preview, and play it directly in the browser via
`magnetite-web-client` — no download required.

---

## Local development loop

Start the full platform stack locally with Docker Compose:

```bash
cp .env.example .env          # fill in JWT_SECRET at minimum
docker compose up -d           # postgres + redis + backend + frontend + mailhog
```

| Service | URL |
|---------|-----|
| Backend API | http://localhost:8080 |
| Frontend | http://localhost:3000 |
| MailHog (email preview) | http://localhost:8025 |
| PgAdmin | http://localhost:5050 |

Healthcheck:

```bash
curl http://localhost:8080/health/ready
```

---

## Next steps

- [Develop in the browser](../moat/develop-in-browser.md) — full Web Studio + in-browser preview guide
- [Moat quickstart](../moat/quickstart.md) — CLI developer walkthrough (full AuthoritativeGame detail)
- [Architecture overview](../moat/architecture-overview.md) — crate map and per-tick pipeline
- [SDK Reference](./sdk.md) — all types and trait methods
- [Build & Distribution Pipeline](./build-pipeline.md) — CI hooks, artifact storage, release flow
- [Architecture Overview](../architecture.md) — backend modules map
- [API Reference](../api-reference/index.md) — REST endpoints
- [MOAT-ARCHITECTURE.md](../MOAT-ARCHITECTURE.md) — frozen interface definitions (source of truth)
