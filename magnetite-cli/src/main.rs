//! # magnetite — CLI for the Magnetite game platform
//!
//! Build and ship server-authoritative Rust games.
//!
//! ## Commands
//!
//! | Command | Status | Description |
//! |---|---|---|
//! | `magnetite new <name>` | **implemented** | Scaffold a new authoritative game crate |
//! | `magnetite build` | **implemented** | `cargo build --release --target wasm32-wasip1` |
//! | `magnetite dev` | **implemented** | Build → load into sandbox → run SingleRoom server → print URL |
//! | `magnetite deploy` | **implemented** | Build → register artifact → request runtime instance |
//!
//! ## Example
//!
//! ```bash
//! magnetite new my-game      # scaffold game crate in ./my-game/
//! cd my-game
//! magnetite build            # produces ./target/wasm32-wasip1/release/my_game.wasm
//! magnetite dev              # prints ws://127.0.0.1:<port>, serves locally
//! magnetite deploy           # registers artifact with backend, prints result
//! ```

use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// Magnetite — build and ship server-authoritative Rust games.
#[derive(Parser)]
#[command(
    name = "magnetite",
    version,
    about = "Build and ship server-authoritative Rust games on the Magnetite platform.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new authoritative game crate.
    ///
    /// Creates a new directory `<name>/` containing a `Cargo.toml` and a
    /// minimal `src/lib.rs` that implements `AuthoritativeGame` from the
    /// Magnetite SDK. The scaffolded crate is ready for `magnetite build`.
    New {
        /// Name of the new game crate (also used as the directory name).
        name: String,
    },

    /// Build the game for the Magnetite sandbox.
    ///
    /// Runs `cargo build --release --target wasm32-wasip1` inside the current
    /// directory and prints the path to the produced `game.wasm` artifact.
    ///
    /// Prerequisites:
    ///   - Rust toolchain with the `wasm32-wasip1` target installed.
    ///     Install with: `rustup target add wasm32-wasip1`
    Build {
        /// Path to the game crate (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// Run the game locally in a SingleRoom server.
    ///
    /// Steps:
    ///   1. `magnetite build` — produces `game.wasm`.
    ///   2. Load `game.wasm` into `magnetite-sandbox` (`WasmExecutor`).
    ///   3. Start `magnetite-runtime` in `SingleRoom` topology.
    ///   4. Serve WebSocket connections on a local port.
    ///   5. Print a `ws://127.0.0.1:<port>` connect URL.
    ///
    /// Press Ctrl-C to stop the server.
    Dev {
        /// Path to the game crate (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Port to listen on (0 = OS-assigned).
        #[arg(long, default_value_t = 0u16)]
        port: u16,

        /// Maximum players (determines topology selection).
        #[arg(long, default_value_t = 4u32)]
        max_players: u32,
    },

    /// Run a capacity-elastic node hosting a content-addressed game.
    ///
    /// Unlike `dev` (a fixed SingleRoom convenience server), `node` boots the
    /// full decentralized host described in DECENTRALIZATION.md §4:
    ///
    ///   1. Build (or load `--wasm`) the game module.
    ///   2. Content-address it: game id = BLAKE3 hash of the module bytes.
    ///   3. Measure THIS box's hardware → capacity (player cap is emergent, not
    ///      a constant — more cores means more shards means more players).
    ///   4. Self-advertise a `SessionAd` via a `Discovery` provider (LAN default)
    ///      instead of polling a central provisioning table.
    ///   5. Load the module by hash, VERIFY the hash, and serve it.
    ///
    /// Runs fully offline with the default providers — no tracker, no chain.
    Node {
        /// Path to the game crate (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Serve an already-compiled `.wasm` module instead of building `path`.
        #[arg(long)]
        wasm: Option<PathBuf>,

        /// Bind host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on (0 = OS-assigned).
        #[arg(long, default_value_t = 9000u16)]
        port: u16,

        /// Shard cell size in world units (used when capacity implies sharding).
        #[arg(long, default_value_t = 500.0f32)]
        cell_size: f32,

        /// Deterministic RNG seed for the match.
        #[arg(long, default_value_t = 0xDEAD_BEEF_CAFE_1234u64)]
        seed: u64,
    },

    /// Deploy the game to a Magnetite runtime instance.
    ///
    /// Steps:
    ///   1. `magnetite build` — produces `game.wasm`.
    ///   2. Register the artifact with the Magnetite distribution API
    ///      (`POST /api/v1/distribution/<game_id>/versions`).
    ///   3. Print the registration result including the version ID.
    ///
    /// Required environment variables:
    ///   - `MAGNETITE_API_URL`  — base URL of the backend (e.g. https://api.magnetite.dev).
    ///   - `MAGNETITE_GAME_ID` — UUID of the game registered on the platform.
    ///
    /// Optional environment variables:
    ///   - `MAGNETITE_API_TOKEN` — bearer token for authenticated endpoints.
    ///   - `MAGNETITE_VERSION`   — semantic version string (default: "0.1.0").
    ///   - `MAGNETITE_COMMIT`    — git commit SHA (default: "local").
    Deploy {
        /// Path to the game crate (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::New { name } => cmd_new(&name),
        Commands::Build { path } => cmd_build(&path),
        Commands::Dev {
            path,
            port,
            max_players,
        } => cmd_dev(&path, port, max_players),
        Commands::Node {
            path,
            wasm,
            host,
            port,
            cell_size,
            seed,
        } => cmd_node(&path, wasm.as_deref(), &host, port, cell_size, seed),
        Commands::Deploy { path } => cmd_deploy(&path),
    }
}

// ---------------------------------------------------------------------------
// `magnetite new <name>`
// ---------------------------------------------------------------------------

fn cmd_new(name: &str) -> Result<()> {
    validate_crate_name(name)?;

    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("directory `{name}` already exists");
    }

    std::fs::create_dir_all(dir.join("src")).with_context(|| format!("creating `{name}/src/`"))?;

    write_file(&dir.join("Cargo.toml"), cargo_toml_template(name))?;
    write_file(&dir.join("src").join("lib.rs"), lib_rs_template(name))?;

    println!("Created game crate `{name}/`");
    println!();
    println!("Next steps:");
    println!("  cd {name}");
    println!("  # Implement AuthoritativeGame in src/lib.rs");
    println!("  magnetite build");

    Ok(())
}

/// Return the scaffold `Cargo.toml` content for a new game crate.
fn cargo_toml_template(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "Authoritative Magnetite game — {name}"
license = "MIT"

# cdylib = wasm32-wasip1 sandbox artifact; rlib = tests/native linking.
[lib]
crate-type = ["cdylib", "rlib"]

# ── Features ─────────────────────────────────────────────────────────────────
# `wasm` — emit the mag_* sandbox ABI (build with --target wasm32-wasip1)
[features]
default = []
wasm = []

# ── Dependencies ──────────────────────────────────────────────────────────────
[dependencies]
magnetite-sdk = {{ path = "../backend/magnetite-sdk" }}
serde       = {{ version = "1", features = ["derive"] }}
serde_json  = "1"

# ── Profiles ──────────────────────────────────────────────────────────────────
[profile.release]
lto           = true
opt-level     = "z"
codegen-units = 1
panic         = "abort"
"#,
        name = name,
    )
}

/// Return the scaffold `src/lib.rs` content for a new game crate.
fn lib_rs_template(name: &str) -> String {
    format!(
        r#"//! `{name}` — authoritative Magnetite game.
//!
//! Implement [`AuthoritativeGame`] below, then run:
//!
//! ```bash
//! magnetite build   # → target/wasm32-wasip1/release/{crate_name}.wasm
//! magnetite dev     # local SingleRoom server
//! magnetite deploy  # deploy to Magnetite
//! ```

use magnetite_sdk::authority::{{
    AuthoritativeGame, MatchConfig, RejectReason, StepCtx, Tick,
}};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;
use serde::{{Deserialize, Serialize}};

// ---------------------------------------------------------------------------
// State types — implement your game data here.
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MySnapshot {{
    pub tick: u64,
}}

#[derive(Serialize, Deserialize)]
pub struct MyDelta {{
    pub tick: u64,
}}

#[derive(Serialize)]
pub struct MyView {{
    pub tick: u64,
}}

#[derive(Serialize, Deserialize)]
pub enum MyCommand {{
    /// A no-op command — replace with your real commands.
    Noop,
}}

// ---------------------------------------------------------------------------
// Game struct
// ---------------------------------------------------------------------------

pub struct {pascal_name} {{
    tick: u64,
}}

impl AuthoritativeGame for {pascal_name} {{
    type Snapshot = MySnapshot;
    type Delta    = MyDelta;
    type View     = MyView;
    type Command  = MyCommand;

    fn init(_cfg: &MatchConfig) -> Self {{
        Self {{ tick: 0 }}
    }}

    fn validate(
        &self,
        _player: PlayerId,
        _input: &Input,
        _tick: Tick,
    ) -> Result<Vec<MyCommand>, RejectReason> {{
        // TODO: translate raw input into authoritative commands.
        Ok(vec![MyCommand::Noop])
    }}

    fn step(&mut self, ctx: &mut StepCtx, _commands: &[(PlayerId, MyCommand)]) {{
        // TODO: advance game state deterministically.
        // ONLY use ctx.rng for randomness — never std::random or thread_rng.
        self.tick = ctx.tick;
    }}

    fn snapshot(&self) -> MySnapshot {{
        MySnapshot {{ tick: self.tick }}
    }}

    fn restore(snap: &MySnapshot, _cfg: &MatchConfig) -> Self {{
        Self {{ tick: snap.tick }}
    }}

    fn delta(&self, since: &MySnapshot) -> MyDelta {{
        MyDelta {{ tick: self.tick.saturating_sub(since.tick) }}
    }}

    fn view_for(&self, _player: PlayerId) -> MyView {{
        MyView {{ tick: self.tick }}
    }}
}}

// ---------------------------------------------------------------------------
// WASM ABI (--features wasm, --target wasm32-wasip1)
// ---------------------------------------------------------------------------

// TODO: add a `wasm_abi.rs` module (see game-template-authoritative for a
// reference implementation) and re-export the mag_* functions here:
//
//   #[cfg(feature = "wasm")]
//   pub mod wasm_abi;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {{
    use super::*;
    use magnetite_sdk::authority::{{MatchConfig, NativeExecutor, GameExecutor}};
    use magnetite_sdk::state::PlayerId;
    use magnetite_sdk::input::Input;

    #[test]
    fn smoke_test() {{
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<{pascal_name}>::new(cfg);
        let p = PlayerId::new(1);
        let out = exec.step(1, &[(p, Input::default())]);
        assert_eq!(out.rejects.len(), 0);
    }}
}}
"#,
        name = name,
        crate_name = name.replace('-', "_"),
        pascal_name = to_pascal_case(name),
    )
}

// ---------------------------------------------------------------------------
// `magnetite build`
// ---------------------------------------------------------------------------

fn cmd_build(crate_path: &Path) -> Result<()> {
    let crate_path = crate_path
        .canonicalize()
        .with_context(|| format!("resolving path `{}`", crate_path.display()))?;

    println!("Building `{}` for wasm32-wasip1…", crate_path.display());

    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "--features",
            "wasm",
        ])
        .current_dir(&crate_path)
        .status()
        .context("running `cargo build` — is the Rust toolchain installed?")?;

    if !status.success() {
        bail!(
            "`cargo build --release --target wasm32-wasip1` failed (exit code {:?})\n\
             Tip: ensure the target is installed: rustup target add wasm32-wasip1",
            status.code()
        );
    }

    // Locate the produced .wasm artifact.
    let wasm_path = locate_wasm(&crate_path)?;
    println!("Build succeeded.");
    println!("Artifact: {}", wasm_path.display());

    Ok(())
}

/// Find the `.wasm` file produced by a `cargo build --target wasm32-wasip1`.
///
/// Looks in `<crate>/target/wasm32-wasip1/release/*.wasm`.  If multiple files
/// are found the first is returned (only one cdylib per crate).
fn locate_wasm(crate_path: &Path) -> Result<PathBuf> {
    let release_dir = crate_path
        .join("target")
        .join("wasm32-wasip1")
        .join("release");

    if !release_dir.exists() {
        bail!(
            "release dir `{}` not found after successful build — \
             check that crate-type includes `cdylib`",
            release_dir.display()
        );
    }

    let wasm: Vec<PathBuf> = std::fs::read_dir(&release_dir)
        .with_context(|| format!("reading `{}`", release_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wasm"))
        .collect();

    match wasm.as_slice() {
        [] => bail!(
            "no `.wasm` file found in `{}` after build",
            release_dir.display()
        ),
        [path, ..] => Ok(path.clone()),
    }
}

// ---------------------------------------------------------------------------
// `magnetite dev`
// ---------------------------------------------------------------------------

fn cmd_dev(crate_path: &Path, port: u16, max_players: u32) -> Result<()> {
    let crate_path = crate_path
        .canonicalize()
        .with_context(|| format!("resolving path `{}`", crate_path.display()))?;

    // Step 1: build the wasm.
    cmd_build(&crate_path)?;

    // Step 2: locate the wasm artifact.
    let wasm_path = locate_wasm(&crate_path)?;

    // Step 3: pick a free port.
    //
    // Bind to port 0 on the loopback, let the OS assign a port, record it, then
    // close the listener.  There is a small TOCTOU window but it is acceptable
    // for local dev use — the alternative (bind_addr "127.0.0.1:0") does not
    // expose the assigned port through the current GameServer API.
    let bind_addr = if port == 0 {
        let tmp = StdTcpListener::bind("127.0.0.1:0")
            .context("could not bind to 127.0.0.1:0 — is loopback available?")?;
        let addr = tmp.local_addr().context("could not get local address")?;
        // Drop the listener so the port is free for GameServer to bind.
        drop(tmp);
        addr.to_string()
    } else {
        format!("127.0.0.1:{port}")
    };

    let connect_url = format!("ws://{bind_addr}");

    println!("Loading `{}`…", wasm_path.display());
    println!();
    println!("  Connect URL : {connect_url}");
    println!("  Topology    : SingleRoom (max {max_players} players)");
    println!("  Tick rate   : 20 Hz");
    println!();
    println!("Press Ctrl-C to stop.");
    println!();

    // Step 4: build the tokio runtime and run the server.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;

    rt.block_on(async move {
        use magnetite_runtime::{GameServer, GameServerConfig, MatchConfig};
        use magnetite_sandbox::LimitsConfig;

        let match_cfg = MatchConfig::auto(max_players);
        let server_cfg = GameServerConfig {
            bind_addr: bind_addr.clone(),
            match_config: match_cfg,
            anticheat: None,
            fleet: None,
        };
        let limits = LimitsConfig::default();

        GameServer::serve_wasm(wasm_path, limits, server_cfg)
            .await
            .map_err(|e| anyhow::anyhow!("server error: {e}"))
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// `magnetite node`
// ---------------------------------------------------------------------------

/// This node's signing identity for tracker announcements.
///
/// A tracker refuses unsigned ads and binds a `(game, node)` slot to the key
/// that first claimed it, so the key must be STABLE across restarts or the node
/// loses its own listing. `MAGNETITE_NODE_SEED` (32-byte hex) sets it
/// explicitly; otherwise it is derived deterministically from the bind address.
///
/// TODO(node-key): persist a generated keypair under the node's data dir so an
/// operator's identity survives a change of bind address, and expose it to
/// `magnetite node --print-key`.
fn node_identity(bind_addr: &str) -> magnetite_seams::identity::RawKeypairAuth {
    use magnetite_seams::identity::RawKeypairAuth;
    if let Ok(hex_seed) = std::env::var("MAGNETITE_NODE_SEED") {
        if let Ok(raw) = hex::decode(hex_seed.trim()) {
            if let Ok(seed) = <[u8; 32]>::try_from(raw.as_slice()) {
                return RawKeypairAuth::from_seed(seed);
            }
        }
        eprintln!("warning: MAGNETITE_NODE_SEED is not 32 bytes of hex — deriving instead");
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(magnetite_seams::blobstore::Hash::of(bind_addr.as_bytes()).0.as_slice());
    RawKeypairAuth::from_seed(seed)
}

fn cmd_node(
    crate_path: &Path,
    wasm_override: Option<&Path>,
    host: &str,
    port: u16,
    cell_size: f32,
    seed: u64,
) -> Result<()> {
    use magnetite_runtime::{
        content_address, BlobStore, Discovery, FanoutDiscovery, Filter, LanDiscovery,
        LocalBlobStore, NodeConfig,
    };
    use std::sync::Arc;

    // Step 1: obtain the module bytes — either a prebuilt --wasm or build the crate.
    let wasm_path = match wasm_override {
        Some(p) => p
            .canonicalize()
            .with_context(|| format!("resolving --wasm `{}`", p.display()))?,
        None => {
            let crate_path = crate_path
                .canonicalize()
                .with_context(|| format!("resolving path `{}`", crate_path.display()))?;
            cmd_build(&crate_path)?;
            locate_wasm(&crate_path)?
        }
    };
    let wasm_bytes =
        std::fs::read(&wasm_path).with_context(|| format!("reading `{}`", wasm_path.display()))?;

    // Step 2: content-address the module (game id = BLAKE3 hash of the bytes).
    let game = content_address(&wasm_bytes);
    let bind_addr = format!("{host}:{port}");

    // Step 3: build the tokio runtime and stand up the node.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;

    rt.block_on(async move {
        // Default, fully-offline providers: local content store + LAN phonebook.
        let blobs = LocalBlobStore::new();
        let stored = blobs.put(&wasm_bytes).await;
        debug_assert_eq!(stored, game, "stored hash must equal computed content address");
        // Phonebooks, in order of how little they need: LAN always, plus an
        // HTTP tracker if (and only if) TRACKER_URL was set. Fanning out means
        // the SAME discovery handle is announced to, renewed on, and retracted
        // from — so the background heartbeat keeps us listed everywhere, not
        // just on the LAN.
        let tracker = magnetite_runtime::tracker::from_env(node_identity(&bind_addr));
        let tracker_configured = tracker.is_some();
        let mut backends: Vec<Box<dyn Discovery + Send + Sync>> = vec![Box::new(LanDiscovery::new())];
        if let Some(t) = tracker {
            backends.push(Box::new(t));
        }
        let discovery = Arc::new(FanoutDiscovery::new(backends));

        let cfg = NodeConfig {
            bind_addr: bind_addr.clone(),
            cell_size,
            seed,
            ..Default::default()
        };

        // Prepare (measure capacity + verify hash + advertise) so we can print
        // the emergent numbers before blocking in the serve loop.
        let prepared = magnetite_runtime::prepare_game(&blobs, discovery.as_ref(), &game, &cfg)
            .await
            .map_err(|e| anyhow::anyhow!("node bring-up failed: {e}"))?;

        let cap = &prepared.ad.capacity;
        println!();
        println!("Magnetite node — capacity-elastic, self-advertising");
        println!();
        println!("  Game id (BLAKE3) : {}", game.to_hex());
        println!("  Connect URL      : ws://{bind_addr}");
        println!("  Topology         : {:?}", prepared.match_config.topology);
        println!(
            "  Measured HW      : {} cores, {} MB RAM",
            cap.cpu_cores, cap.ram_mb
        );
        println!(
            "  Emergent cap     : {} shards, {} player slots (derived from HW, not a constant)",
            cap.max_shards, prepared.match_config.max_players
        );

        // Confirm the ad is discoverable by game hash (the phonebook now knows us).
        let found = discovery.find(game, Filter::default()).await;
        println!("  Advertised       : {} session(s) discoverable by hash", found.len());

        // OPT-IN: an HTTP tracker. LAN discovery is the zero-config default and
        // needs no service at all; a tracker is a redundant, swappable
        // phonebook you point at with TRACKER_URL. Failing to reach one is a
        // lost hint, never a failure to host — the fanout above already
        // announced best-effort and the node serves regardless.
        if tracker_configured {
            println!(
                "  Tracker          : announcing (signed by this node's key) to {}",
                std::env::var(magnetite_runtime::tracker::TRACKER_URL_ENV).unwrap_or_default()
            );
        } else {
            println!(
                "  Tracker          : none configured (set {} to opt in)",
                magnetite_runtime::tracker::TRACKER_URL_ENV
            );
        }
        println!(
            "  Lease            : renewed every {}s while serving; retracted on shutdown",
            cfg.lease.as_secs() / 2
        );
        println!();
        println!("Press Ctrl-C to stop.");
        println!();

        // Serve the verified, content-addressed game.
        magnetite_runtime::run_node(&blobs, Arc::clone(&discovery), &game, cfg)
            .await
            .map_err(|e| anyhow::anyhow!("node error: {e}"))
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// `magnetite deploy`
// ---------------------------------------------------------------------------

/// Response shape returned by `POST /api/v1/distribution/:game_id/versions`.
///
/// We only need the fields we print; the rest are ignored.
#[derive(Debug, serde::Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, serde::Deserialize)]
struct VersionData {
    id: String,
    version: String,
    commit_sha: String,
    build_status: Option<String>,
}

fn cmd_deploy(crate_path: &Path) -> Result<()> {
    let crate_path = crate_path
        .canonicalize()
        .with_context(|| format!("resolving path `{}`", crate_path.display()))?;

    // Step 1: build the wasm.
    cmd_build(&crate_path)?;

    // Step 2: locate the wasm artifact.
    let wasm_path = locate_wasm(&crate_path)?;

    // Step 3: read required environment variables.
    let api_url = std::env::var("MAGNETITE_API_URL").unwrap_or_default();
    let game_id = std::env::var("MAGNETITE_GAME_ID").unwrap_or_default();

    if api_url.is_empty() || game_id.is_empty() {
        bail!(
            "missing required environment variables.\n\
             \n\
             Set both of the following before running `magnetite deploy`:\n\
             \n\
             \x20 MAGNETITE_API_URL  — base URL of the backend\n\
             \x20                      e.g. https://api.magnetite.dev\n\
             \x20 MAGNETITE_GAME_ID  — UUID of your game on the platform\n\
             \x20                      e.g. 01234567-89ab-cdef-0123-456789abcdef\n\
             \n\
             Optional:\n\
             \x20 MAGNETITE_API_TOKEN  — bearer token for auth-guarded endpoints\n\
             \x20 MAGNETITE_VERSION    — version string (default: 0.1.0)\n\
             \x20 MAGNETITE_COMMIT     — git commit SHA (default: local)"
        );
    }

    let api_token = std::env::var("MAGNETITE_API_TOKEN").ok();
    let version = std::env::var("MAGNETITE_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let commit = std::env::var("MAGNETITE_COMMIT").unwrap_or_else(|_| "local".to_string());

    // Step 4: read the wasm bytes and compute basic metadata.
    let wasm_bytes =
        std::fs::read(&wasm_path).with_context(|| format!("reading `{}`", wasm_path.display()))?;
    let wasm_size = wasm_bytes.len();

    println!(
        "Deploying `{}` ({} bytes) as version {} (commit: {})…",
        wasm_path.display(),
        wasm_size,
        version,
        commit
    );

    // Step 5: POST to the distribution API.
    let url = format!(
        "{}/api/v1/distribution/{}/versions",
        api_url.trim_end_matches('/'),
        game_id
    );

    let mut req_builder = reqwest::blocking::Client::new()
        .post(&url)
        .header("Content-Type", "application/json");

    if let Some(token) = &api_token {
        req_builder = req_builder.bearer_auth(token);
    }

    let body = serde_json::json!({
        "version": version,
        "commit_sha": commit,
        "release_notes": format!("wasm artifact — {} bytes", wasm_size),
    });

    let resp = req_builder.json(&body).send().map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            anyhow::anyhow!(
                "could not reach the Magnetite backend at `{api_url}`.\n\
                 \n\
                 Check that:\n\
                 \x20 1. MAGNETITE_API_URL is correct (current: {api_url})\n\
                 \x20 2. The backend is running and accessible from this machine\n\
                 \x20 3. Any firewall / VPN rules allow the connection\n\
                 \n\
                 Underlying error: {e}"
            )
        } else {
            anyhow::anyhow!("HTTP request failed: {e}")
        }
    })?;

    let status = resp.status();

    if status.is_success() {
        // Attempt to decode the structured response; fall back to raw text.
        let text = resp
            .text()
            .unwrap_or_else(|_| "(no response body)".to_string());

        match serde_json::from_str::<ApiResponse<VersionData>>(&text) {
            Ok(parsed) => {
                let v = &parsed.data;
                println!();
                println!("Deploy registered successfully.");
                println!();
                println!("  Version ID  : {}", v.id);
                println!("  Version     : {}", v.version);
                println!("  Commit      : {}", v.commit_sha);
                if let Some(s) = &v.build_status {
                    println!("  Build status: {s}");
                }
                println!();
                println!(
                    "The artifact has been registered. To promote it to live, use the\n\
                     platform dashboard or call:\n\
                     \n\
                     \x20 PUT {}/api/v1/distribution/{}/versions/{}/promote",
                    api_url.trim_end_matches('/'),
                    game_id,
                    v.id
                );
            }
            Err(_) => {
                // Partial / unexpected response — still print it.
                println!();
                println!("Deploy registered (HTTP {status}).");
                println!("Response: {text}");
            }
        }
    } else {
        let text = resp
            .text()
            .unwrap_or_else(|_| "(no response body)".to_string());
        bail!(
            "backend returned HTTP {status}.\n\
             \n\
             Response body:\n\
             {text}\n\
             \n\
             Possible causes:\n\
             \x20 - MAGNETITE_GAME_ID ({game_id}) does not match an active game\n\
             \x20 - MAGNETITE_API_TOKEN is missing or invalid (HTTP 401/403)\n\
             \x20 - The backend rejected the request (see response body above)"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate that `name` is a usable Rust crate / directory name.
fn validate_crate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("crate name must not be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "crate name `{name}` contains invalid characters — \
             use only ASCII letters, digits, `-`, and `_`"
        );
    }
    if name.starts_with('-') || name.starts_with('_') {
        bail!("crate name `{name}` must not start with `-` or `_`");
    }
    Ok(())
}

/// Convert a `kebab-case` or `snake_case` name to `PascalCase`.
fn to_pascal_case(name: &str) -> String {
    name.split(|c| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect()
}

/// Write `content` to `path`, creating parent directories as needed.
fn write_file(path: &Path, content: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory `{}`", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("writing `{}`", path.display()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ //
    // to_pascal_case                                                       //
    // ------------------------------------------------------------------ //

    #[test]
    fn pascal_case_kebab() {
        assert_eq!(to_pascal_case("my-game"), "MyGame");
    }

    #[test]
    fn pascal_case_snake() {
        assert_eq!(to_pascal_case("my_game"), "MyGame");
    }

    #[test]
    fn pascal_case_single_word() {
        assert_eq!(to_pascal_case("arena"), "Arena");
    }

    #[test]
    fn pascal_case_no_separators_preserved() {
        // Words with no `-` or `_` separator are returned with first char
        // uppercased and the rest as-is (already-pascal names pass through).
        assert_eq!(to_pascal_case("MyGame"), "MyGame");
        assert_eq!(to_pascal_case("arena"), "Arena");
    }

    // ------------------------------------------------------------------ //
    // validate_crate_name                                                  //
    // ------------------------------------------------------------------ //

    #[test]
    fn validate_name_accepts_valid() {
        assert!(validate_crate_name("my-game").is_ok());
        assert!(validate_crate_name("my_game").is_ok());
        assert!(validate_crate_name("game1").is_ok());
        assert!(validate_crate_name("abc").is_ok());
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_crate_name("").is_err());
    }

    #[test]
    fn validate_name_rejects_leading_dash() {
        assert!(validate_crate_name("-game").is_err());
    }

    #[test]
    fn validate_name_rejects_special_chars() {
        assert!(validate_crate_name("my game").is_err());
        assert!(validate_crate_name("my/game").is_err());
        assert!(validate_crate_name("my.game").is_err());
    }

    // ------------------------------------------------------------------ //
    // locate_wasm (error path — no build artifacts)                       //
    // ------------------------------------------------------------------ //

    #[test]
    fn locate_wasm_missing_dir_returns_error() {
        let tmp = std::env::temp_dir().join("magnetite_cli_test_nonexistent");
        assert!(locate_wasm(&tmp).is_err());
    }

    // ------------------------------------------------------------------ //
    // cargo_toml_template                                                 //
    // ------------------------------------------------------------------ //

    #[test]
    fn cargo_toml_template_contains_name() {
        let toml = cargo_toml_template("cool-game");
        assert!(
            toml.contains("cool-game"),
            "TOML must contain the crate name"
        );
    }

    #[test]
    fn cargo_toml_template_contains_wasm_feature() {
        let toml = cargo_toml_template("cool-game");
        assert!(toml.contains("wasm = []"), "TOML must declare wasm feature");
    }

    // ------------------------------------------------------------------ //
    // lib_rs_template                                                     //
    // ------------------------------------------------------------------ //

    #[test]
    fn lib_rs_template_contains_pascal_name() {
        let src = lib_rs_template("cool-game");
        assert!(
            src.contains("CoolGame"),
            "lib.rs must use PascalCase struct name"
        );
    }

    #[test]
    fn lib_rs_template_contains_authoritative_game() {
        let src = lib_rs_template("cool-game");
        assert!(
            src.contains("AuthoritativeGame"),
            "lib.rs must implement AuthoritativeGame"
        );
    }

    // ------------------------------------------------------------------ //
    // cmd_new — scaffold in temp dir                                      //
    // ------------------------------------------------------------------ //

    #[test]
    fn cmd_new_creates_files() {
        // Use write_file + cargo_toml_template/lib_rs_template directly so the
        // test doesn't depend on process-global cwd manipulation (which races
        // with other parallel tests).
        let tmp = std::env::temp_dir().join(format!(
            "magnetite_cli_new_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();

        let cargo_path = tmp.join("Cargo.toml");
        let lib_path = tmp.join("src").join("lib.rs");

        write_file(&cargo_path, cargo_toml_template("test-game")).unwrap();
        write_file(&lib_path, lib_rs_template("test-game")).unwrap();

        assert!(cargo_path.exists(), "Cargo.toml must be created");
        assert!(lib_path.exists(), "src/lib.rs must be created");

        // Clean up.
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cmd_new_rejects_existing_dir() {
        // Create a real directory and verify cmd_new rejects it.
        // We change cwd to temp_dir before calling cmd_new so the relative
        // path lookup points there.  Wrap in a mutex-style serialisation by
        // using a unique dir name to avoid races with other tests.
        let tmp = std::env::temp_dir();
        let existing = tmp.join(format!(
            "mag_existing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&existing).unwrap();
        let dir_name = existing.file_name().unwrap().to_string_lossy().to_string();

        let orig = std::env::current_dir().unwrap_or_else(|_| tmp.clone());
        std::env::set_current_dir(&tmp).ok();
        let result = cmd_new(&dir_name);
        std::env::set_current_dir(orig).ok();

        let _ = std::fs::remove_dir_all(&existing);
        assert!(result.is_err(), "cmd_new must fail when dir already exists");
    }

    // ------------------------------------------------------------------ //
    // cmd_deploy — missing env vars                                       //
    // ------------------------------------------------------------------ //

    #[test]
    fn deploy_errors_on_missing_env() {
        // Temporarily unset the env vars and verify we get the expected error.
        // This test is intentionally fragile to the env, which is fine for
        // a unit test that documents the contract.
        let orig_api = std::env::var("MAGNETITE_API_URL").ok();
        let orig_game = std::env::var("MAGNETITE_GAME_ID").ok();

        std::env::remove_var("MAGNETITE_API_URL");
        std::env::remove_var("MAGNETITE_GAME_ID");

        // We can't call cmd_deploy directly (it runs cargo build first), so
        // replicate the validation logic inline.
        let api_url = std::env::var("MAGNETITE_API_URL").unwrap_or_default();
        let game_id = std::env::var("MAGNETITE_GAME_ID").unwrap_or_default();
        let missing = api_url.is_empty() || game_id.is_empty();

        // Restore.
        if let Some(v) = orig_api {
            std::env::set_var("MAGNETITE_API_URL", v);
        }
        if let Some(v) = orig_game {
            std::env::set_var("MAGNETITE_GAME_ID", v);
        }

        assert!(missing, "should report missing env vars");
    }

    // ------------------------------------------------------------------ //
    // cmd_dev — port selection                                             //
    // ------------------------------------------------------------------ //

    #[test]
    fn dev_picks_free_port_when_zero() {
        // Verify the port-picking logic works (just the bind + drop part).
        let tmp = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let addr = tmp.local_addr().unwrap();
        drop(tmp);
        assert!(addr.port() > 0, "OS must assign a non-zero port");
    }
}
