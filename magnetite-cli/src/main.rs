//! # magnetite — CLI for the Magnetite game platform
//!
//! Build and ship server-authoritative Rust games.
//!
//! ## Commands (N1)
//!
//! | Command | Status | Description |
//! |---|---|---|
//! | `magnetite new <name>` | **implemented** | Scaffold a new authoritative game crate |
//! | `magnetite build` | **implemented** | `cargo build --release --target wasm32-wasip1` |
//! | `magnetite dev` | stub (N2) | Build → load into sandbox → run SingleRoom server → print URL |
//! | `magnetite deploy` | stub (N2) | Build → register artifact → request runtime instance |
//!
//! ## Example
//!
//! ```bash
//! magnetite new my-game      # scaffold game crate in ./my-game/
//! cd my-game
//! magnetite build            # produces ./target/wasm32-wasip1/release/my_game.wasm
//! magnetite dev              # N2 — prints "implemented in N2"
//! magnetite deploy           # N2 — prints "implemented in N2"
//! ```

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

    /// Run the game locally in a SingleRoom server. (Implemented in N2.)
    ///
    /// Intended steps (N2):
    ///   1. `magnetite build` to produce `game.wasm`.
    ///   2. Load `game.wasm` into `magnetite-sandbox` (`WasmExecutor`).
    ///   3. Start `magnetite-runtime` in `SingleRoom` topology.
    ///   4. Serve WebSocket connections on a local port.
    ///   5. Print a `ws://localhost:<port>` connect URL.
    Dev,

    /// Deploy the game to a Magnetite runtime instance. (Implemented in N2.)
    ///
    /// Intended steps (N2):
    ///   1. `magnetite build` to produce `game.wasm`.
    ///   2. Register the artifact with the Magnetite distribution API.
    ///   3. Request a runtime instance for the artifact.
    ///   4. Print the WebSocket connect URL for the live server.
    Deploy,
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
        Commands::Dev => cmd_dev(),
        Commands::Deploy => cmd_deploy(),
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
//! magnetite dev     # local SingleRoom server (N2)
//! magnetite deploy  # deploy to Magnetite (N2)
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
// `magnetite dev` (N2 stub)
// ---------------------------------------------------------------------------

fn cmd_dev() -> Result<()> {
    println!("magnetite dev — implemented in N2");
    println!();
    println!("Intended steps (N2):");
    println!("  1. magnetite build → game.wasm");
    println!("  2. Load game.wasm into magnetite-sandbox (WasmExecutor)");
    println!("  3. Start magnetite-runtime in SingleRoom topology");
    println!("  4. Serve WebSocket connections on a local port");
    println!("  5. Print ws://localhost:<port> connect URL");
    Ok(())
}

// ---------------------------------------------------------------------------
// `magnetite deploy` (N2 stub)
// ---------------------------------------------------------------------------

fn cmd_deploy() -> Result<()> {
    println!("magnetite deploy — implemented in N2");
    println!();
    println!("Intended steps (N2):");
    println!("  1. magnetite build → game.wasm");
    println!("  2. Register artifact with the Magnetite distribution API");
    println!("  3. Request a runtime instance for the artifact");
    println!("  4. Print the WebSocket connect URL for the live server");
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
}
