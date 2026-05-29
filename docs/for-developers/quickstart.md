# Developer Quickstart

Get from zero to a published Rust game on Magnetite in five steps.

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `wasm-pack` | latest | `cargo install wasm-pack` |
| `wasm-bindgen-cli` | 0.2.x | `cargo install wasm-bindgen-cli` |
| Node.js | 18+ LTS | https://nodejs.org |
| Docker + Compose | 24+ / 2.20+ | https://docs.docker.com |

Add the WASM target once:

```bash
rustup target add wasm32-unknown-unknown
```

---

## Step 1 — Clone the game template

```bash
git clone https://github.com/magnetite-platform/game-template my-game
cd my-game
```

The template is a minimal Bevy + `magnetite-sdk` crate that already compiles to WASM:

```
my-game/
├── Cargo.toml          # [lib] crate-type = ["cdylib", "rlib"]
├── build.sh            # cargo build --target wasm32 → wasm-bindgen
├── index.html          # browser harness
└── src/
    └── lib.rs          # GamePlugin + GameLogic impl (start here)
```

---

## Step 2 — Implement `GameLogic`

Open `src/lib.rs`. The core trait you implement is:

```rust
use magnetite_sdk::{GameLogic, GameMetadata, Input, GameState, PlayerId};

impl GameLogic for MyGameState {
    fn new() -> Self { /* initialise world */ }

    fn handle_input(&mut self, player: PlayerId, input: Input) -> magnetite_sdk::Action {
        // read input.keys (forward/backward/left/right/jump/crouch/attack)
        // and input.mouse (x, y, delta_x, delta_y, buttons)
        // mutate self, return an Action
    }

    fn tick(&mut self) {
        // advance simulation one step (called at metadata().tick_rate Hz)
    }

    fn state(&self) -> &GameState {
        // return a reference to the canonical GameState the platform broadcasts to clients
        &self.game_state
    }

    fn players(&self) -> Vec<PlayerId> { self.players.keys().cloned().collect() }

    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "my-game".into(),
            max_players: 4,
            tick_rate: 60,
        }
    }
}
```

Extend `PlayerState.custom` (a `serde_json::Value`) with anything game-specific —
the platform always has access to the common fields (position, rotation, health, score)
without knowing your internals.

See the [SDK Reference](./sdk.md) for all types.

---

## Step 3 — Build to WASM

```bash
# inside my-game/
bash build.sh
```

`build.sh` runs:

```bash
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen \
  --target web \
  --out-dir ./dist \
  --out-name game \
  target/wasm32-unknown-unknown/release/my_game.wasm
```

Output: `dist/game_bg.wasm` + `dist/game.js` bindings.

Open `index.html` in a browser (via a local HTTP server — browsers block `file://` WASM) to
smoke-test:

```bash
npx serve .
# or
python3 -m http.server 8000
```

---

## Step 4 — Register your repository

Point Magnetite at your source so the CI pipeline can build and verify your game.

**Via the dashboard** (recommended for first-time setup):

1. Sign in → Developer Portal → **Register Repository**.
2. Install the Magnetite GitHub App on your repo.
3. The platform records the installation and monitors push events.

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

---

## Step 5 — Publish your game

Once CI is green, publish through the Developer Portal or the API:

```bash
# 1. Register the game record
curl -X POST https://api.magnetite.gg/api/v1/games \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "My Game",
    "description": "…",
    "max_players": 4,
    "tick_rate": 60,
    "wasm_artifact_url": "https://…/game_bg.wasm"
  }'

# 2. Submit for review (admin approves before going live)
PUT /api/v1/developer/games/{id}/status
{ "status": "pending_review" }
```

After admin approval the game appears in the marketplace and players can discover and play it.

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

- [SDK Reference](./sdk.md) — all types and trait methods
- [Build & Distribution Pipeline](./build-pipeline.md) — CI hooks, artifact storage, release flow
- [Architecture Overview](../architecture.md) — backend modules map
- [API Reference](../api-reference/index.md) — REST endpoints
