# In-Browser Studio Code Editor

This document covers the **Game Studio** UI at `/studio` — the browser-based
entry point for creating and deploying Magnetite games. It complements the
lower-level [`develop-in-browser.md`](develop-in-browser.md), which covers the
`magnetite dev` / `magnetite-web-client` runtime protocol in detail.

---

## What is the Studio?

The Studio (`src/pages/GameStudio.jsx`) is a React page that walks a developer
through three steps in the browser:

1. **Choose a Template** — select from the registered game templates (fetched
   via `GET /api/v1/templates`).
2. **Configure** — enter a game name and optional description.
3. **Get Started** — registers the game (`POST /api/v1/games`) and returns CLI
   commands plus a live in-browser preview panel.

No local toolchain is required to reach Step 3. The CLI commands shown
(`magnetite install`, `magnetite build`, `magnetite dev`, `magnetite deploy`)
can be executed locally or inside a Codespace.

---

## Templates

Templates are returned by `GET /api/v1/templates` and are compiled from the
`game-templates/arcade/`, `game-templates/fps/`, and `game-templates/motorsport/`
workspace crates. The current set:

| Template slug | Description | Topology | Tick rate |
|---|---|---|---|
| `minimal` | Bare `AuthoritativeGame` stub | SingleRoom | 20 Hz |
| `arena-shooter` | Top-down arena reference game | SingleRoom | 60 Hz |
| `fps-starter` | First-person movement skeleton | SingleRoom | 60 Hz |
| `motorsport` | Track + lap counter + collision hull | Dedicated | 30 Hz |

All templates implement the `AuthoritativeGame` trait and export the WASM ABI
(`mag_init`, `mag_step`, `mag_snapshot`, `mag_restore`, `mag_view`). Selecting
a template in the Studio shows its player count, tick rate, and topology before
committing.

---

## Step-by-step flow

### Step 1 — Choose Template

The Studio fetches templates on mount via `GET /api/v1/templates` (no auth
required). Each card shows:

- Template name and a short description.
- Maximum player count, tick rate, and topology.
- A **Select template** button.

Clicking a card advances to Step 2.

### Step 2 — Configure

Fields:

| Field | Constraints | Notes |
|---|---|---|
| Game Name | Required; alphanumeric + hyphens | Used as the Cargo crate name |
| Description | Optional | Shown on the marketplace listing |

The template selection is shown as a read-only summary. A **Back** button
returns to Step 1.

### Step 3 — Get Started

On submit the Studio calls `POST /api/v1/games` with:

```json
{
  "title": "My Game",
  "description": "Optional description",
  "category": "action"
}
```

A success response returns a `game_id` UUID. The Studio then renders:

- **CLI Setup** — copy-paste commands to scaffold and connect the local repo:

  ```bash
  cargo install magnetite-cli
  magnetite new my-game --template arena-shooter --game-id <uuid>
  cd my-game
  magnetite build
  magnetite dev
  ```

- **Preview in Browser** — an embedded WebSocket preview panel (`GamePreview`
  component). Enter the `ws://` URL printed by `magnetite dev` and click
  **Play in Browser** to connect `magnetite-web-client` to the running server.

- **Connect GitHub and Deploy** — links to the deploy page where GitHub
  installations are managed.

---

## In-browser preview

The preview panel is the `GamePreview` component rendered inside the Studio's
Step 3 view. It:

1. Accepts a `ws://` (or `wss://`) server URL from the developer.
2. Opens a `magnetite-web-client` `MagnetiteClient` pointed at that URL.
3. Renders the game view onto a `<canvas>` element in the page.

This is the same web client used by the play pages — no extra tooling needed.
The developer sees exactly what players will see.

For the full protocol reference (frames, prediction, renderer API) see
[`develop-in-browser.md`](develop-in-browser.md).

---

## GitHub integration

The Studio's **Connect GitHub** button on Step 3 calls
`GET /api/v1/github/installations`. When no installation is found the page
shows a link to install the Magnetite GitHub App on the repo. Once connected,
every push to the default branch triggers a build status update visible in the
developer dashboard.

The build trigger path:

```
GitHub push event
  → POST /api/v1/github/webhook
  → trigger_wasm_build()
  → writes status='building' row in DB
  → (Bucket D) CI runner invokes wasm-pack / cargo build --target wasm32-wasip1
```

The final WASM artifact upload step requires an external CI runner. See
[`docs/self-hosting/local-infra.md`](../self-hosting/local-infra.md) and
[`docs/self-hosting/external-dependencies.md`](../self-hosting/external-dependencies.md)
for setup instructions.

---

## Ownership notes

| File | Role |
|---|---|
| `src/pages/GameStudio.jsx` | Studio page (template picker + configure + get-started) |
| `src/components/GamePreview.jsx` | Embedded WS preview canvas |
| `backend/src/api/templates.rs` | `GET /api/v1/templates` |
| `backend/src/api/games.rs` | `POST /api/v1/games` (create game, returns game_id) |
| `backend/src/api/github.rs` | GitHub App webhook + installation endpoints |
| `magnetite-web-client/src/` | Browser WebSocket client (ServerNet/ClientNet protocol) |

---

## Further reading

- [`develop-in-browser.md`](develop-in-browser.md) — full runtime protocol and
  `magnetite dev` details.
- [`quickstart.md`](quickstart.md) — end-to-end CLI-first walkthrough.
- [`architecture-overview.md`](architecture-overview.md) — crate map.
- [`docs/for-developers/sdk.md`](../for-developers/sdk.md) — SDK reference.
- [`docs/for-developers/build-pipeline.md`](../for-developers/build-pipeline.md)
  — WASM build pipeline.
