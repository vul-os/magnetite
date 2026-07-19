# Game Submission

Submit your game to a Magnetite storefront for review and listing.

> **Submission is optional.** Listing on a storefront is one way to distribute
> a game, not a requirement for running one. A game is a content-addressed
> WASM module: `magnetite dev` runs it with no backend, and `magnetite node`
> hosts it on your own hardware and self-advertises to whatever discovery
> tracker you point it at — no review queue involved. This page describes the
> curated-storefront path.

## CI

CI runs as a **GitHub Actions workflow**, not a Magnetite-specific manifest.
The canonical reference is `.github/workflows/game-ci.yml` in this repository;
copy it into your game repo and adjust. There is no `.magnetite/ci.yaml` and no
`magnetite.yaml` manifest — those do not exist.

### Jobs in `game-ci.yml`

| Job | What it runs |
|-----|--------------|
| `check` | `cargo check` + `cargo clippy` (host, `--no-default-features`) |
| `test` | `cargo test` (host, `--no-default-features`) |
| `build-wasm` | `cargo build --release --target wasm32-unknown-unknown --features wasm`, then `wasm-bindgen`, then `wasm-opt -Oz`, then uploads the deploy artifact |
| `audit` | `cargo audit` — currently non-blocking (`|| true`) |

### Registering with the platform

Register your repository with the Magnetite GitHub App
(`POST /api/v1/github/repos/register`) so push and check events reach the
backend and build status is tracked. Artifacts are registered against a game
version through the distribution API — see
[Build & Distribution Pipeline](./build-pipeline.md).

## Security Scan

> ⚠️ **Status: not implemented.** The backend's `run_security_scan` is a stub
> that records a build-log line and returns; it performs **no analysis**. The
> `cargo audit` CI job is non-blocking. The patterns below describe the
> *intended* policy, not an enforced one.
>
> What actually constrains a game is the **runtime sandbox**, which is real:
> games execute inside `magnetite-sandbox` (Wasmtime) under fuel, memory, and
> epoch limits with WASI stubbed, so filesystem, network, environment, and
> process access are unavailable at execution time regardless of what the
> source contains. See [Security & Sandboxing](../security/index.md).

### Prohibited Patterns (intended policy)

```rust
// ❌ Forbidden: File system access
std::fs::read("secrets.txt")

// ❌ Forbidden: Network requests
reqwest::get("https://evil.com")

// ❌ Forbidden: Environment reading
std::env::var("API_KEY")

// ❌ Forbidden: Arbitrary code execution
std::process::Command::new("rm").arg("-rf").spawn()
```

### Allowed APIs

```rust
// ✅ Allowed: Standard math
std::math::sin(x)

// ✅ Allowed: Game logic
HashMap::new()
Vec::push()

// ✅ Allowed: Logging
log::info!("Player moved")
```

### Running Security Checks

```bash
# Check dependencies for known vulnerabilities
cargo audit
```

There is no `magnetite security` subcommand. The CLI's commands are
`new`, `build`, `dev`, `node`, and `deploy`.

## Review Process

### Submission Flow

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  SUBMIT  │────►│  BUILD   │────►│ SECURITY │────►│  REVIEW  │
└──────────┘     └──────────┘     └──────────┘     └────┬─────┘
                                                          │
                         ┌──────────┐     ┌──────────┐     │
                         │  LIVE    │◄────│ APPROVED │◄────┘
                         └──────────┘     └──────────┘
```

Review turnaround is a policy each storefront operator sets, not a platform
guarantee. Anyone can run a node and a storefront; there is no single "platform
team" gating the network.

### Submitting

There is no `magnetite submit` command. The real path:

```bash
# 1. Build the WASM artifact
magnetite build

# 2. Register it as a version with a storefront's distribution API
#    (MAGNETITE_API_URL, MAGNETITE_GAME_ID, MAGNETITE_API_TOKEN)
magnetite deploy
```

Then move the game into review from the developer portal or with
`PUT /api/v1/developer/games/:id/status`. The operator's admin acts on it via
`PUT /api/v1/admin/games/:id/review` and `/approve`.

### Metadata

Game metadata lives in the platform's `games` record (created through
`POST /api/v1/games` or the developer portal) and in
`magnetite_sdk::game::GameMetadata` returned by your game code. There is no
`magnetite.yaml` file — that manifest does not exist. Note also that entry-fee
and prize-pool fields are not part of the shipped model; paid access is
receipt-gated (see [Payments](../payments.md)).

## Versioning

### Semantic Versioning

```
major.minor.patch
1.0.0
```

| Component | Change Type | Example |
|-----------|-------------|---------|
| major | Breaking | 1.0.0 → 2.0.0 |
| minor | New feature | 1.0.0 → 1.1.0 |
| patch | Bug fix | 1.0.0 → 1.0.1 |

### Update Process

Version promotion and rollback are **API operations**, not CLI subcommands.
`magnetite update`, `magnetite rollback`, `magnetite metrics`, and
`magnetite stats` do not exist.

```bash
# Register a new version (bump MAGNETITE_VERSION, then)
magnetite deploy

# Promote it live
curl -X PUT "$MAGNETITE_API_URL/api/v1/developer/games/$GAME_ID/versions/$VERSION_ID/promote" \
  -H "Authorization: Bearer $MAGNETITE_API_TOKEN"

# Roll back
curl -X PUT "$MAGNETITE_API_URL/api/v1/developer/games/$GAME_ID/versions/$VERSION_ID/rollback" \
  -H "Authorization: Bearer $MAGNETITE_API_TOKEN"
```

## Post-Deployment

### Monitoring

| Surface | Endpoint |
|---------|----------|
| Per-game analytics | `GET /api/v1/developer/games/:id/analytics` |
| Developer dashboard | `GET /api/v1/developer/dashboard` |
| Build status | `GET /api/v1/developer/games/:id/build-status` |
| Node metrics (Prometheus) | `GET /metrics` |

### Hotfixes

There is no `--hotfix` flag and no review-bypass path. A hotfix is an ordinary
version: build, `magnetite deploy`, promote. Whether promotion requires a
review step is the storefront operator's policy.
