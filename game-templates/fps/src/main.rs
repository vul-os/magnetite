//! # Magnetite FPS Starter — Native Binary Entry Point
//!
//! Compiles only when the `native` feature is enabled.
//!
//! ```bash
//! cargo run --features native
//! ```
//!
//! This launches the Bevy + rapier3d desktop window with:
//! - First-person camera and player controller
//! - Level geometry from [`magnetite_fps_starter::level`]
//! - Local game loop driving [`magnetite_fps_starter::FpsGame`]
//! - Gamepad (controller) support via Bevy + gilrs

fn main() {
    #[cfg(feature = "native")]
    magnetite_fps_starter::run_native();

    #[cfg(not(feature = "native"))]
    {
        eprintln!(
            "magnetite-fps-starter: the native desktop entry point requires the `native` feature."
        );
        eprintln!("  Run with:  cargo run --features native");
        std::process::exit(1);
    }
}
