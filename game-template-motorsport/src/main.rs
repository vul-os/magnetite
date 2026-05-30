//! Native binary entry point for Circuit Rush.
//!
//! Run with:
//! ```
//! cargo run --features native
//! ```

#[cfg(feature = "native")]
fn main() {
    magnetite_game_motorsport::run_native();
}

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("Circuit Rush: run with --features native for the desktop client.");
    eprintln!("For WASM: use build.sh or:");
    eprintln!("  cargo build --target wasm32-unknown-unknown --no-default-features --features wasm");
}
