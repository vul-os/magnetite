//! # game-client-bevy
//!
//! Reference **Bevy client** for the Magnetite authoritative server, demonstrating:
//!
//! * **Client-side prediction** — inputs are applied locally each frame so the
//!   game feels responsive even at high latency.
//! * **Server reconciliation** — when the server sends an [`ServerNet::Ack`] the
//!   client discards acknowledged frames from the [`PredictionBuffer`] and
//!   re-simulates the remaining unacked inputs on top of the authoritative state.
//! * **Snapshot / delta application** — [`ServerNet::Snapshot`] replaces the
//!   local view; [`ServerNet::Delta`] is applied incrementally.
//!
//! ## Module layout
//!
//! | Module | Purpose |
//! |---|---|
//! | [`prediction`] | `ClientPredictor` — the core reconcile loop (no Bevy, fully testable) |
//! | [`net`] | WebSocket I/O task (tokio / ewebsock), message queue |
//! | [`app`] | Bevy app wiring + 2-D arena renderer (requires `render` feature) |
//!
//! ## Netcode protocol
//!
//! The client speaks the MOAT netcode wire protocol defined in
//! `magnetite_sdk::protocol::{ClientNet, ServerNet}`:
//!
//! ```text
//! Client                       Server
//!   │                             │
//!   │── ClientNet::InputFrame ───>│   (every client tick, seq++, push to PredictionBuffer)
//!   │                             │
//!   │<── ServerNet::Welcome ──────│   (on connect — sets player_id + MatchConfig)
//!   │<── ServerNet::Snapshot ─────│   (every snapshot_every ticks — full state)
//!   │<── ServerNet::Delta ────────│   (every tick — interest-filtered diff)
//!   │<── ServerNet::Ack ──────────│   (server processed seq → discard + reconcile)
//!   │<── ServerNet::Reject ───────│   (bad input — force reconcile)
//! ```
//!
//! ## Prediction / reconciliation
//!
//! 1. Each frame: `ClientPredictor::predict(input)` → applies input locally,
//!    records it in the [`PredictionBuffer`], returns an `InputFrame` to send.
//! 2. On `Ack { seq, tick }`: `ClientPredictor::reconcile_ack(seq, authoritative_view)` →
//!    acknowledges frames ≤ seq, re-runs remaining unacked inputs on top of the
//!    authoritative state.
//! 3. On `Snapshot { tick, full }`: `ClientPredictor::reconcile_snapshot(snap)` →
//!    replaces authoritative state wholesale, re-runs all unacked inputs.

pub mod net;
pub mod prediction;

#[cfg(feature = "render")]
pub mod app;
