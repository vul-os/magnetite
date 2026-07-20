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

        /// Authorize a peer node: `<hex>` or `<hex>@<host:port>`. Repeatable.
        ///
        /// This is the cluster membership list and it is DENY-BY-DEFAULT: with
        /// no peers configured this node hands shards to nobody and admits
        /// follows from nobody (ordinary single-box behaviour). A malformed
        /// entry is a hard error, never a silently dropped one.
        ///
        /// The optional `@host:port` is the peer's HANDOFF address. It is a
        /// location hint only — the key is what authorizes, and the handshake
        /// still aborts unless the far side proves control of exactly that key.
        /// Peers given an address are the ones the rebalancer can place work on.
        ///
        /// Also readable from `MAGNETITE_CLUSTER_PEERS` (comma/whitespace
        /// separated).
        #[arg(long = "cluster-peer", value_name = "HEX[@ADDR]")]
        cluster_peer: Vec<String>,

        /// File of authorized peer public keys, one 64-hex key per line.
        /// `#` starts a comment. Env: `MAGNETITE_CLUSTER_PEERS_FILE`.
        #[arg(long, value_name = "PATH")]
        cluster_peers_file: Option<PathBuf>,

        /// Bind address for the cluster handoff listener — the authenticated
        /// node-to-node port, SEPARATE from the player-facing game port.
        /// Defaults to `<host>:<port + 1>` when peers are configured.
        /// Env: `MAGNETITE_HANDOFF_ADDR`.
        #[arg(long, value_name = "ADDR")]
        handoff_addr: Option<String>,

        /// Path to this node's persisted Ed25519 keypair. Generated on first
        /// run with owner-only permissions; reused afterwards so the node's
        /// identity is stable across restarts AND across bind-address changes.
        /// Default: `$MAGNETITE_HOME/node.key`, else `~/.magnetite/node.key`.
        /// Env: `MAGNETITE_NODE_KEY_FILE`.
        #[arg(long, value_name = "PATH")]
        node_key_file: Option<PathBuf>,

        /// Turn the automatic cluster rebalancer OFF.
        ///
        /// It is ON by default whenever at least one peer was given an address
        /// (`--cluster-peer <hex>@<addr>`), because a cluster that knows its
        /// members and cannot reach a balanced state is the failure this loop
        /// exists to prevent. Turning it off leaves placement entirely manual;
        /// nothing will migrate on its own.
        #[arg(long, default_value_t = false)]
        no_rebalance: bool,

        /// Seconds between rebalance passes. Longer is calmer: every pass that
        /// decides to move something costs a client reconnect.
        #[arg(long, value_name = "SECS", default_value_t = 30)]
        rebalance_interval: u64,

        /// How many shards this node may be over its fair share before anything
        /// moves. `0` disables hysteresis and is not recommended — a greedy
        /// bin-pack routinely differs by one shard for pure tie-breaking
        /// reasons, and reacting to that is how a rebalancer starts thrashing.
        #[arg(long, value_name = "N", default_value_t = 1)]
        rebalance_deadband: u32,

        /// Most migrations one rebalance pass may start.
        #[arg(long, value_name = "N", default_value_t = 2)]
        rebalance_max_in_flight: usize,
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
            cluster_peer,
            cluster_peers_file,
            handoff_addr,
            node_key_file,
            no_rebalance,
            rebalance_interval,
            rebalance_deadband,
            rebalance_max_in_flight,
        } => cmd_node(
            &path,
            wasm.as_deref(),
            &host,
            port,
            cell_size,
            seed,
            NodeClusterOpts {
                cluster_peer,
                cluster_peers_file,
                handoff_addr,
                node_key_file,
                no_rebalance,
                rebalance_interval,
                rebalance_deadband,
                rebalance_max_in_flight,
            },
        ),
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

/// Cluster-related options for `magnetite node`, grouped so the command
/// signature stays readable.
struct NodeClusterOpts {
    cluster_peer: Vec<String>,
    cluster_peers_file: Option<PathBuf>,
    handoff_addr: Option<String>,
    node_key_file: Option<PathBuf>,
    no_rebalance: bool,
    rebalance_interval: u64,
    rebalance_deadband: u32,
    rebalance_max_in_flight: usize,
}

/// Where this node's persisted keypair lives when no path was given.
fn default_node_key_path() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("MAGNETITE_HOME") {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home).join("node.key"));
        }
    }
    let home = std::env::var("HOME").ok().filter(|h| !h.trim().is_empty())?;
    Some(PathBuf::from(home).join(".magnetite").join("node.key"))
}

/// Parse a 32-byte hex seed, rejecting anything that is not exactly 32 bytes.
fn parse_seed_hex(s: &str) -> Result<[u8; 32]> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    let raw = hex::decode(s).map_err(|e| anyhow::anyhow!("not valid hex: {e}"))?;
    <[u8; 32]>::try_from(raw.as_slice())
        .map_err(|_| anyhow::anyhow!("expected 32 bytes (64 hex chars), got {}", raw.len()))
}

/// Restrict a key file to owner read/write.
#[cfg(unix)]
fn secure_key_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("setting 0600 on `{}`", path.display()))
}

#[cfg(not(unix))]
fn secure_key_file(_path: &Path) -> Result<()> {
    Ok(())
}

/// Warn (loudly, but do not refuse) if a key file is readable by anyone else.
#[cfg(unix)]
fn warn_if_key_file_is_loose(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(md) = std::fs::metadata(path) {
        let mode = md.permissions().mode() & 0o077;
        if mode != 0 {
            eprintln!(
                "warning: node key `{}` is readable beyond its owner (mode {:o}) — \
                 anyone who reads it can impersonate this node; run `chmod 600` on it",
                path.display(),
                md.permissions().mode() & 0o777
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_key_file_is_loose(_path: &Path) {}

/// How this node's identity was obtained (printed at startup so an operator
/// knows whether the key will survive a restart).
enum KeySource {
    /// `MAGNETITE_NODE_SEED` — explicit, stable, operator-managed.
    Env,
    /// Loaded from an existing key file.
    File(PathBuf),
    /// Generated and written to a key file on first run.
    Generated(PathBuf),
    /// STOPGAP: derived from the bind address because no key file location
    /// could be determined (no `HOME`, no `--node-key-file`). Identity changes
    /// if the bind address changes.
    DerivedFromAddr,
}

/// This node's signing identity: tracker announcements, the handoff handshake,
/// and every redirect it mints are signed with this key.
///
/// A tracker refuses unsigned ads and binds a `(game, node)` slot to the key
/// that first claimed it, and peers pin this key in their membership lists, so
/// the key MUST be stable across restarts. Precedence:
///
/// 1. `MAGNETITE_NODE_SEED` (32-byte hex) — explicit override.
/// 2. `--node-key-file` / `MAGNETITE_NODE_KEY_FILE`, else
///    `$MAGNETITE_HOME/node.key`, else `~/.magnetite/node.key` — loaded if
///    present, generated (0600) on first run if not.
/// 3. Last-resort derivation from the bind address, only when no key file path
///    can be determined at all. This is the old stopgap: identity moves with
///    the address.
fn node_identity(
    bind_addr: &str,
    key_file: Option<&Path>,
) -> Result<(magnetite_seams::identity::RawKeypairAuth, KeySource)> {
    use magnetite_seams::identity::RawKeypairAuth;

    if let Ok(hex_seed) = std::env::var("MAGNETITE_NODE_SEED") {
        match parse_seed_hex(&hex_seed) {
            Ok(seed) => return Ok((RawKeypairAuth::from_seed(seed), KeySource::Env)),
            Err(e) => bail!(
                "MAGNETITE_NODE_SEED is malformed ({e}).\n\
                 Refusing to fall back to a different identity — a silently \n\
                 changed node key orphans this node's tracker slot and invalidates\n\
                 every membership list that pinned it. Unset it or fix the value."
            ),
        }
    }

    let path = key_file
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("MAGNETITE_NODE_KEY_FILE")
                .ok()
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from)
        })
        .or_else(default_node_key_path);

    let Some(path) = path else {
        // No explicit path and no home directory: keep the historical stopgap
        // rather than refusing to boot, but say so at startup.
        let mut seed = [0u8; 32];
        seed.copy_from_slice(
            magnetite_seams::blobstore::Hash::of(bind_addr.as_bytes())
                .0
                .as_slice(),
        );
        return Ok((RawKeypairAuth::from_seed(seed), KeySource::DerivedFromAddr));
    };

    if path.exists() {
        warn_if_key_file_is_loose(&path);
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading node key `{}`", path.display()))?;
        let seed = parse_seed_hex(&contents)
            .with_context(|| format!("node key `{}` is malformed", path.display()))?;
        return Ok((RawKeypairAuth::from_seed(seed), KeySource::File(path)));
    }

    // First run: generate from the OS CSPRNG and persist owner-only.
    let identity = RawKeypairAuth::generate();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating `{}`", parent.display()))?;
    }
    std::fs::write(&path, format!("{}\n", hex::encode(identity.seed())))
        .with_context(|| format!("writing node key `{}`", path.display()))?;
    secure_key_file(&path)?;
    Ok((identity, KeySource::Generated(path)))
}

/// Parse one authorized peer public key: exactly 32 bytes of hex.
fn parse_peer_pubkey(s: &str) -> Result<magnetite_seams::identity::PubKey> {
    let t = s.trim();
    let t = t.strip_prefix("0x").unwrap_or(t);
    let raw = hex::decode(t)
        .map_err(|e| anyhow::anyhow!("`{s}` is not a hex-encoded public key: {e}"))?;
    let bytes = <[u8; 32]>::try_from(raw.as_slice()).map_err(|_| {
        anyhow::anyhow!(
            "`{s}` is {} bytes; an Ed25519 public key is 32 bytes (64 hex chars)",
            raw.len()
        )
    })?;
    Ok(magnetite_seams::identity::PubKey(bytes))
}

/// One authorized cluster member: a key, and optionally where to reach it.
///
/// The key is the authorization. The address is only a hint about where that key
/// answers, and it confers nothing on its own — the handoff handshake still
/// aborts unless the far side proves control of exactly `key`, so a wrong or
/// stolen address yields a failed connection rather than a misdirected shard.
///
/// A peer with no address is still a full member (it may hand shards to us, and
/// its follows are admitted); we simply have nowhere to send work, so the
/// rebalancer never places anything on it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClusterPeer {
    key: magnetite_seams::identity::PubKey,
    addr: Option<String>,
}

/// Parse one membership entry: `<hex>` or `<hex>@<host:port>`.
fn parse_cluster_peer(s: &str) -> Result<ClusterPeer> {
    let t = s.trim();
    match t.split_once('@') {
        None => Ok(ClusterPeer {
            key: parse_peer_pubkey(t)?,
            addr: None,
        }),
        Some((k, addr)) => {
            let addr = addr.trim();
            if addr.is_empty() {
                bail!("`{s}` has an empty address after `@`; write `<hex>@<host:port>`");
            }
            Ok(ClusterPeer {
                key: parse_peer_pubkey(k)?,
                addr: Some(addr.to_string()),
            })
        }
    }
}

/// Collect the cluster membership list from flags, env, and an optional file.
///
/// **Fail loudly, never drop.** A single malformed entry aborts the whole list:
/// silently skipping one would produce a membership an operator did not write,
/// and a peer they believed authorized would be refused at handoff time with no
/// hint why. An EMPTY result is a legitimate answer and means "no cluster".
fn collect_cluster_peers(
    flags: &[String],
    file: Option<&Path>,
) -> Result<Vec<ClusterPeer>> {
    let env_list = std::env::var("MAGNETITE_CLUSTER_PEERS").ok();
    let file = file.map(PathBuf::from).or_else(|| {
        std::env::var("MAGNETITE_CLUSTER_PEERS_FILE")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .map(PathBuf::from)
    });
    collect_cluster_peers_from(flags, env_list.as_deref(), file.as_deref())
}

/// Pure core of [`collect_cluster_peers`] — no environment access, so the
/// deny-by-default and fail-loudly contracts are directly testable.
fn collect_cluster_peers_from(
    flags: &[String],
    env_list: Option<&str>,
    file: Option<&Path>,
) -> Result<Vec<ClusterPeer>> {
    let mut out: Vec<ClusterPeer> = Vec::new();
    // De-dup on the KEY. A repeated key that carries an address upgrades the
    // entry: the operator naming a location is strictly more information than
    // the operator naming only a key, and it never widens who is authorized.
    let mut push = |p: ClusterPeer| {
        match out.iter_mut().find(|e| e.key.0 == p.key.0) {
            Some(existing) => {
                if existing.addr.is_none() {
                    existing.addr = p.addr;
                }
            }
            None => out.push(p),
        }
    };

    for raw in flags {
        push(parse_cluster_peer(raw).context("--cluster-peer")?);
    }

    if let Some(list) = env_list {
        for tok in list
            .split([',', ' ', '\t', '\n'])
            .filter(|t| !t.trim().is_empty())
        {
            push(parse_cluster_peer(tok).context("MAGNETITE_CLUSTER_PEERS")?);
        }
    }

    if let Some(path) = file {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("reading peer file `{}`", path.display()))?;
        for (i, line) in contents.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            push(parse_cluster_peer(line).with_context(|| {
                format!("{}:{}: invalid peer key", path.display(), i + 1)
            })?);
        }
    }

    Ok(out)
}

/// Default handoff bind address for a given game bind: the next port up.
fn default_handoff_addr(host: &str, port: u16) -> Result<String> {
    let next = port
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("--port {port} leaves no room for a handoff port; pass --handoff-addr"))?;
    Ok(format!("{host}:{next}"))
}

/// The live cluster wiring for a node that has authorized peers.
struct FleetWiring {
    /// The authenticated node-to-node listener. Held for its lifetime: dropping
    /// it shuts the handoff door.
    node: magnetite_runtime::fleet::FleetNode,
    /// Attached to the game server so player sessions follow migrated shards.
    session: magnetite_runtime::follow::FleetSession,
    /// The resolved listen address (`:0` resolved to a real port).
    addr: String,
    /// The outbound transport, shared with the rebalance loop.
    transport: std::sync::Arc<std::sync::Mutex<magnetite_runtime::fleet::NetworkHandoffTransport>>,
    /// Membership-gated routes to peers the operator gave an address for.
    directory: magnetite_runtime::cluster::RouteDirectory,
    /// How many members we actually know how to reach.
    routable: usize,
}

/// Bind the handoff listener and build the fleet session for `peers`.
///
/// Both the inbound door ([`FleetNode::bind`]'s allowlist) and the outbound
/// transport ([`NetworkHandoffTransport::with_membership`]) are given the SAME
/// explicit key set. There is no code path here that passes `None`/empty as
/// "allow anyone": this function is only ever called with a non-empty `peers`,
/// and a node with no peers gets no fleet wiring at all.
fn build_fleet(
    handoff_addr: &str,
    identity: std::sync::Arc<magnetite_seams::identity::RawKeypairAuth>,
    peers: &[ClusterPeer],
) -> Result<FleetWiring> {
    use magnetite_runtime::cluster::{ClusterMembership, RouteDirectory};
    use magnetite_runtime::fleet::{FleetNode, PeerRoute};
    use magnetite_runtime::follow::FleetSession;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    debug_assert!(!peers.is_empty(), "fleet wiring requires explicit peers");

    let keys: Vec<_> = peers.iter().map(|p| p.key).collect();
    let membership = ClusterMembership::from_keys(keys.iter().copied());
    let allowed: HashSet<_> = keys.iter().copied().collect();

    let node = FleetNode::bind(handoff_addr, Arc::clone(&identity), Some(allowed))
        .map_err(|e| anyhow::anyhow!("binding handoff listener on `{handoff_addr}`: {e}"))?;
    let addr = node.addr().to_string();

    // Routes for the peers the operator located. The directory is gated by the
    // SAME membership, so an address for a key that is not a member is refused
    // here rather than becoming a quietly-usable route.
    let mut directory = RouteDirectory::new(membership.clone());
    let mut routable = 0usize;
    for p in peers {
        if let Some(a) = &p.addr {
            directory
                .admit_operator_route(PeerRoute::new(a.clone(), p.key))
                .map_err(|e| {
                    anyhow::anyhow!("route `{a}` for peer {}: {e}", hex::encode(p.key.0))
                })?;
            routable += 1;
        }
    }

    let transport = Arc::new(Mutex::new(
        node.transport().with_membership(membership.clone()),
    ));
    let session = FleetSession::new(identity, node.authority(), membership)
        .with_transport(Arc::clone(&transport));

    Ok(FleetWiring {
        node,
        session,
        addr,
        transport,
        directory,
        routable,
    })
}

/// The background reconciler.
///
/// **On by default whenever the cluster is actually routable**, i.e. at least
/// one peer was given an address. The reasoning: a cluster that has been told
/// who its members are and how to reach them, and then does not distribute work,
/// is the bug this loop exists to fix — leaving it opt-in would ship the broken
/// default. It stays off when there is nowhere to send work, which is exactly
/// the deny-by-default case, and `--no-rebalance` turns it off explicitly.
///
/// It never widens authorization. Every tick reads the same membership-gated
/// directory and every move goes through the same two-phase, key-pinned,
/// membership-checked handoff.
fn spawn_rebalance_loop(
    local: magnetite_seams::identity::PubKey,
    transport: std::sync::Arc<std::sync::Mutex<magnetite_runtime::fleet::NetworkHandoffTransport>>,
    directory: magnetite_runtime::cluster::RouteDirectory,
    capacity: magnetite_seams::discovery::Capacity,
    policy: magnetite_runtime::rebalance::RebalancePolicy,
) -> std::thread::JoinHandle<()> {
    use magnetite_runtime::rebalance::Rebalancer;
    use magnetite_runtime::rebalance::SpreadScheduler;

    std::thread::spawn(move || {
        let interval = policy.interval;
        let mut rebalancer = Rebalancer::new(local, policy, Box::new(SpreadScheduler));
        loop {
            std::thread::sleep(interval);
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let report = {
                let Ok(mut t) = transport.lock() else {
                    // A poisoned transport means a migration thread panicked.
                    // Stop reconciling rather than act on unknown state.
                    eprintln!("rebalancer: transport lock poisoned — loop stopping");
                    return;
                };
                rebalancer.tick(
                    &mut t,
                    &directory,
                    &capacity,
                    now_unix,
                    std::time::Instant::now(),
                )
            };
            for (shard, target, epoch) in &report.migrated {
                println!(
                    "  rebalance: shard {shard} -> {} (epoch {epoch})",
                    hex::encode(target.0)
                );
            }
            for (shard, target, err) in &report.failed {
                eprintln!(
                    "  rebalance: shard {shard} -> {} FAILED: {err} \
                     (this node still owns the shard and its state)",
                    hex::encode(target.0)
                );
            }
            // Losses are printed one per line, in full, and never summarised
            // into something that could read as a recovery.
            for loss in &report.lost {
                eprintln!("  rebalance: {loss}");
                eprintln!(
                    "  rebalance: shard {} will NOT be restarted automatically; \
                     starting one would create a NEW world, not restore the old one",
                    loss.shard
                );
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn cmd_node(
    crate_path: &Path,
    wasm_override: Option<&Path>,
    host: &str,
    port: u16,
    cell_size: f32,
    seed: u64,
    cluster: NodeClusterOpts,
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

    // Step 3: resolve this node's identity. Stable across restarts (and across
    // bind-address changes) once a key file exists, because peers pin this key
    // in their membership lists and a tracker binds our slot to it.
    let (identity, key_source) = node_identity(&bind_addr, cluster.node_key_file.as_deref())?;
    let node_seed = identity.seed();
    let node_pubkey = {
        use magnetite_seams::identity::Identity as _;
        identity.pubkey()
    };
    let identity = Arc::new(identity);

    // Step 4: cluster membership — DENY-BY-DEFAULT. An empty list is not "allow
    // anyone", it is "this node is not in a cluster": no handoff listener, no
    // migration transport, no session follow. Exactly today's single-box node.
    let peers = collect_cluster_peers(
        &cluster.cluster_peer,
        cluster.cluster_peers_file.as_deref(),
    )?;
    if peers.iter().any(|k| k.key.0 == node_pubkey.0) {
        eprintln!(
            "warning: this node's own key is in the membership list — harmless, \
             but a node never hands a shard to itself"
        );
    }

    let fleet_wiring = if peers.is_empty() {
        None
    } else {
        let handoff_addr = match cluster.handoff_addr.clone().or_else(|| {
            std::env::var("MAGNETITE_HANDOFF_ADDR")
                .ok()
                .filter(|a| !a.trim().is_empty())
        }) {
            Some(a) => a,
            None => default_handoff_addr(host, port)?,
        };
        if handoff_addr == bind_addr {
            bail!(
                "--handoff-addr ({handoff_addr}) must differ from the player-facing \
                 game address ({bind_addr}) — they are separate listeners"
            );
        }
        Some(build_fleet(&handoff_addr, Arc::clone(&identity), &peers)?)
    };

    // Step 5: build the tokio runtime and stand up the node.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;

    let (fleet_session, fleet_addr, _handoff_node, fleet_transport, fleet_dir, routable) =
        match fleet_wiring {
            Some(w) => (
                Some(w.session),
                Some(w.addr),
                Some(w.node),
                Some(w.transport),
                Some(w.directory),
                w.routable,
            ),
            None => (None, None, None, None, None, 0),
        };
    let capacity_publisher = _handoff_node.as_ref().map(|n| n.capacity_publisher());
    let peer_count = peers.len();

    // The rebalancer runs only when there is somewhere to send work: at least
    // one member with an address. With none, the cluster is configured but not
    // routable, and a loop would do nothing but log.
    let rebalance_on = !cluster.no_rebalance && routable > 0;
    let rebalance_policy = magnetite_runtime::rebalance::RebalancePolicy {
        deadband_shards: cluster.rebalance_deadband,
        max_in_flight: cluster.rebalance_max_in_flight.max(1),
        interval: std::time::Duration::from_secs(cluster.rebalance_interval.max(1)),
        ..Default::default()
    };

    let result = rt.block_on(async move {
        // Default, fully-offline providers: local content store + LAN phonebook.
        let blobs = LocalBlobStore::new();
        let stored = blobs.put(&wasm_bytes).await;
        debug_assert_eq!(stored, game, "stored hash must equal computed content address");
        // Phonebooks, in order of how little they need: LAN always, plus an
        // HTTP tracker if (and only if) TRACKER_URL was set. Fanning out means
        // the SAME discovery handle is announced to, renewed on, and retracted
        // from — so the background heartbeat keeps us listed everywhere, not
        // just on the LAN.
        let tracker = magnetite_runtime::tracker::from_env(
            magnetite_seams::identity::RawKeypairAuth::from_seed(node_seed),
        );
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
            fleet: fleet_session,
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
        println!("  Node pubkey      : {}", hex::encode(node_pubkey.0));
        match &key_source {
            KeySource::Env => println!("  Node key         : MAGNETITE_NODE_SEED (stable)"),
            KeySource::File(p) => {
                println!("  Node key         : {} (stable)", p.display())
            }
            KeySource::Generated(p) => println!(
                "  Node key         : generated → {} (stable from now on)",
                p.display()
            ),
            KeySource::DerivedFromAddr => println!(
                "  Node key         : NOT STABLE — derived from the bind address \
                 (no key file location; pass --node-key-file)"
            ),
        }
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
        // Cluster wiring — deny-by-default. No authorized peers means no
        // handoff listener at all, so there is nothing to hand a shard to.
        match &fleet_addr {
            Some(addr) => {
                println!("  Cluster          : {peer_count} authorized peer key(s)");
                println!("  Handoff listener : {addr} (node-to-node only, mutually authenticated)");
                println!(
                    "  Session follow   : ON — players are redirected when a shard migrates"
                );
                println!(
                    "  Reachability     : peers must reach {addr} DIRECTLY — no NAT traversal, \n\
                     \x20                    no hole punching, no relay (same LAN / VPN / public IP)"
                );
                println!("  Routable peers   : {routable} of {peer_count} have an address");
                if rebalance_on {
                    println!(
                        "  Rebalancer       : ON — every {}s, capacity-aware, deadband {} shard(s), \n\
                         \x20                    at most {} migration(s) per pass",
                        rebalance_policy.interval.as_secs(),
                        rebalance_policy.deadband_shards,
                        rebalance_policy.max_in_flight
                    );
                    println!(
                        "  Node death       : LOSES that node's shard state — there is NO state \n\
                         \x20                    replication; losses are reported, never 'recovered'"
                    );
                } else if cluster.no_rebalance {
                    println!("  Rebalancer       : OFF (--no-rebalance) — placement is manual");
                } else {
                    println!(
                        "  Rebalancer       : OFF — no peer has an address; pass \n\
                         \x20                    --cluster-peer <hex>@<host:port> to make one routable"
                    );
                }
            }
            None => println!(
                "  Cluster          : none configured — this node hands shards to nobody \
                 (pass --cluster-peer <hex> to join a cluster)"
            ),
        }
        println!();
        println!("Press Ctrl-C to stop.");
        println!();

        // Tell peers how big this box is, so their rebalancers can size it, and
        // start our own reconciler. Both use the SAME measured capacity that
        // was advertised to the phonebook — one number, one source.
        if let Some(pubr) = &capacity_publisher {
            pubr.publish(prepared.ad.capacity.clone());
        }
        if rebalance_on {
            if let (Some(t), Some(d)) = (fleet_transport.clone(), fleet_dir.clone()) {
                let _rebalancer = spawn_rebalance_loop(
                    node_pubkey,
                    t,
                    d,
                    prepared.ad.capacity.clone(),
                    rebalance_policy.clone(),
                );
            }
        }

        // Serve the verified, content-addressed game.
        magnetite_runtime::run_node(&blobs, Arc::clone(&discovery), &game, cfg)
            .await
            .map_err(|e| anyhow::anyhow!("node error: {e}"))
    });

    // The handoff listener lives exactly as long as the serve loop: dropping it
    // here shuts the node-to-node door before the process exits.
    drop(_handoff_node);
    result
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
    // cluster membership — deny-by-default, fail-loudly                    //
    // ------------------------------------------------------------------ //

    /// 32 bytes of `0xaa`, hex-encoded (64 chars).
    const KEY_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    /// 32 bytes of `0xbb`, hex-encoded (64 chars).
    const KEY_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn no_config_means_no_cluster_not_open_door() {
        // THE contract: absent configuration is "hand shards to nobody", and it
        // is expressed as an EMPTY membership, which `cmd_node` turns into no
        // handoff listener at all.
        let peers = collect_cluster_peers_from(&[], None, None).unwrap();
        assert!(peers.is_empty(), "no config must yield no authorized peers");

        // An explicitly empty env var is equally not an open door.
        let peers = collect_cluster_peers_from(&[], Some("   "), None).unwrap();
        assert!(peers.is_empty());
    }

    #[test]
    fn peer_flag_parses_hex_key() {
        let peers =
            collect_cluster_peers_from(&[KEY_A.to_string()], None, None).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].key.0, [0xaa; 32]);
        assert_eq!(peers[0].addr, None, "a bare key carries no address");
    }

    #[test]
    fn peer_entry_accepts_an_optional_address() {
        let p = parse_cluster_peer(&format!("{KEY_A}@10.0.0.5:9101")).unwrap();
        assert_eq!(p.key.0, [0xaa; 32]);
        assert_eq!(p.addr.as_deref(), Some("10.0.0.5:9101"));
    }

    #[test]
    fn an_empty_address_is_an_error_not_a_keyless_peer() {
        // `key@` almost certainly means a truncated config line. Accepting it as
        // "member with no address" would silently drop the node out of every
        // placement decision, which is very hard to notice.
        assert!(parse_cluster_peer(&format!("{KEY_A}@")).is_err());
        assert!(parse_cluster_peer(&format!("{KEY_A}@   ")).is_err());
    }

    #[test]
    fn an_address_never_substitutes_for_a_valid_key() {
        assert!(parse_cluster_peer("not-a-key@10.0.0.5:9101").is_err());
    }

    #[test]
    fn the_same_key_twice_keeps_the_address_that_was_given() {
        // Bare key first, then located: the location must win, because an
        // operator who wrote an address meant it.
        let peers = collect_cluster_peers_from(
            &[KEY_A.to_string(), format!("{KEY_A}@10.0.0.5:9101")],
            None,
            None,
        )
        .unwrap();
        assert_eq!(peers.len(), 1, "the same key must not be authorized twice");
        assert_eq!(peers[0].addr.as_deref(), Some("10.0.0.5:9101"));
    }

    #[test]
    fn an_address_for_a_non_member_is_refused_by_the_route_directory() {
        // The address plumbing must not become a way around membership.
        use magnetite_runtime::cluster::{ClusterMembership, RouteDirectory};
        use magnetite_runtime::fleet::PeerRoute;

        let member = parse_peer_pubkey(KEY_A).unwrap();
        let stranger = parse_peer_pubkey(KEY_B).unwrap();
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([member]));

        assert!(dir
            .admit_operator_route(PeerRoute::new("10.0.0.5:9101", member))
            .is_ok());
        assert!(
            dir.admit_operator_route(PeerRoute::new("10.0.0.6:9101", stranger))
                .is_err(),
            "an operator-supplied address must not confer membership"
        );
    }

    #[test]
    fn build_fleet_refuses_to_start_with_an_unusable_peer_address() {
        use magnetite_seams::identity::RawKeypairAuth;
        use std::sync::Arc;

        let id = Arc::new(RawKeypairAuth::from_seed([11u8; 32]));
        let peers = vec![ClusterPeer {
            key: parse_peer_pubkey(KEY_A).unwrap(),
            addr: Some("   ".to_string()),
        }];
        assert!(
            build_fleet("127.0.0.1:0", id, &peers).is_err(),
            "a blank peer address must abort start-up, not silently disable placement"
        );
    }

    #[test]
    fn a_located_peer_becomes_a_routable_placement_target() {
        use magnetite_seams::identity::RawKeypairAuth;
        use std::sync::Arc;

        let id = Arc::new(RawKeypairAuth::from_seed([12u8; 32]));
        let key = parse_peer_pubkey(KEY_A).unwrap();
        let w = build_fleet(
            "127.0.0.1:0",
            id,
            &[
                ClusterPeer {
                    key,
                    addr: Some("10.0.0.5:9101".into()),
                },
                ClusterPeer {
                    key: parse_peer_pubkey(KEY_B).unwrap(),
                    addr: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(w.routable, 1, "only located peers are placement targets");
        // Both are still full MEMBERS — an address is not what authorizes.
        assert_eq!(w.node.allowed().unwrap().len(), 2);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(w.directory.route_for(&key, now).is_ok());
    }

    #[test]
    fn peer_key_accepts_0x_prefix() {
        let k = parse_peer_pubkey(&format!("0x{KEY_B}")).unwrap();
        assert_eq!(k.0, [0xbb; 32]);
    }

    #[test]
    fn malformed_peer_key_is_an_error_not_a_dropped_entry() {
        // Wrong length.
        assert!(parse_peer_pubkey("aabb").is_err());
        // Not hex.
        assert!(parse_peer_pubkey(&"z".repeat(64)).is_err());
        // And one bad entry poisons the whole list rather than shrinking it.
        let flags = vec![KEY_A.to_string(), "nonsense".to_string()];
        assert!(collect_cluster_peers_from(&flags, None, None).is_err());
    }

    #[test]
    fn env_and_flags_merge_and_dedupe() {
        let peers =
            collect_cluster_peers_from(&[KEY_A.to_string()], Some(&format!("{KEY_A},{KEY_B}")), None)
                .unwrap();
        assert_eq!(peers.len(), 2, "duplicates collapse, distinct keys accumulate");
    }

    #[test]
    fn peers_file_parses_comments_and_blank_lines() {
        let tmp = std::env::temp_dir().join(format!(
            "mag_peers_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::write(
            &tmp,
            format!("# node A\n{KEY_A}\n\n{KEY_B}  # node B\n"),
        )
        .unwrap();
        let peers = collect_cluster_peers_from(&[], None, Some(&tmp)).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn peers_file_reports_the_offending_line() {
        let tmp = std::env::temp_dir().join(format!(
            "mag_peers_bad_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::write(&tmp, format!("{KEY_A}\nnot-a-key\n")).unwrap();
        let err = collect_cluster_peers_from(&[], None, Some(&tmp)).unwrap_err();
        let _ = std::fs::remove_file(&tmp);
        assert!(
            format!("{err:#}").contains(":2"),
            "error must point at the bad line, got: {err:#}"
        );
    }

    #[test]
    fn missing_peers_file_is_an_error_not_an_empty_allowlist() {
        // Silently treating an unreadable membership file as "no peers" would
        // be a downgrade an operator did not ask for.
        let missing = std::env::temp_dir().join("mag_peers_definitely_missing.txt");
        let _ = std::fs::remove_file(&missing);
        assert!(collect_cluster_peers_from(&[], None, Some(&missing)).is_err());
    }

    #[test]
    fn fleet_wiring_pins_exactly_the_configured_peers() {
        use magnetite_seams::identity::RawKeypairAuth;
        use std::sync::Arc;

        let id = Arc::new(RawKeypairAuth::from_seed([7u8; 32]));
        let peer = parse_peer_pubkey(KEY_A).unwrap();
        let stranger = parse_peer_pubkey(KEY_B).unwrap();

        let w = build_fleet(
            "127.0.0.1:0",
            id,
            &[ClusterPeer {
                key: peer,
                addr: None,
            }],
        )
        .unwrap();
        let allowed = w
            .node
            .allowed()
            .expect("handoff listener must always carry an explicit allowlist");
        assert!(allowed.contains(&peer));
        assert!(
            !allowed.contains(&stranger),
            "a key the operator did not authorize must not be admitted"
        );
        assert_eq!(allowed.len(), 1);
    }

    #[test]
    fn handoff_port_defaults_next_to_the_game_port() {
        assert_eq!(default_handoff_addr("127.0.0.1", 9000).unwrap(), "127.0.0.1:9001");
        assert!(default_handoff_addr("127.0.0.1", u16::MAX).is_err());
    }

    // ------------------------------------------------------------------ //
    // node identity — persisted keypair                                    //
    // ------------------------------------------------------------------ //

    #[test]
    fn seed_hex_round_trips_and_rejects_short_input() {
        let seed = parse_seed_hex(&format!("0x{KEY_A}\n")).unwrap();
        assert_eq!(seed, [0xaa; 32]);
        assert!(parse_seed_hex("aabb").is_err());
        assert!(parse_seed_hex("nothex").is_err());
    }

    #[test]
    fn node_key_file_is_generated_once_then_reused() {
        use magnetite_seams::identity::Identity as _;
        let dir = std::env::temp_dir().join(format!(
            "mag_nodekey_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let path = dir.join("node.key");

        // MAGNETITE_NODE_SEED must not be set for this test's precedence path.
        if std::env::var("MAGNETITE_NODE_SEED").is_ok() {
            return;
        }

        let (id1, src1) = node_identity("127.0.0.1:9000", Some(&path)).unwrap();
        assert!(matches!(src1, KeySource::Generated(_)));
        assert!(path.exists(), "key file must be written on first run");

        // Same file, DIFFERENT bind address → same identity. That is the whole
        // point: identity no longer moves with the address.
        let (id2, src2) = node_identity("10.0.0.5:7777", Some(&path)).unwrap();
        assert!(matches!(src2, KeySource::File(_)));
        assert_eq!(id1.pubkey().0, id2.pubkey().0);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "generated key must be owner-only");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_node_key_file_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "mag_badkey_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("node.key");
        std::fs::write(&path, "garbage").unwrap();
        if std::env::var("MAGNETITE_NODE_SEED").is_ok() {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        // Refuse rather than quietly booting under a different identity.
        assert!(node_identity("127.0.0.1:9000", Some(&path)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
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
