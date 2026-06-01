//! # `magnetite-e2e`
//!
//! End-to-end integration tests and scale harness for the Magnetite MOAT stack.
//!
//! ## What is proven here
//!
//! | Test | Assertion |
//! |---|---|
//! | `convergence` | N simulated WS clients drive K ticks; all clients observe the same authoritative state hash; `verify_replay` returns `Clean`. |
//! | `anticheat_teleport` | One client sends a teleport/speedhack input; the server rejects it; the cheater's TrustScore escalates. |
//! | `scale_bench` (`#[ignore]`) | Ramp from SingleRoom → Dedicated; measure ticks/sec + per-tick latency; print report. |
//!
//! ## Shared helpers
//!
//! This crate exposes helpers used by both integration tests and the bench binary.

pub mod harness;
