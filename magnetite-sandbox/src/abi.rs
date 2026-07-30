//! Sandbox ABI codec — encoding/decoding between host Rust types and the
//! length-prefixed byte buffers that cross the Wasm linear memory boundary.
//!
//! ## Wire format
//!
//! Every guest-owned buffer uses a **4-byte little-endian length prefix**:
//!
//! ```text
//! [ len_lo, len_hi, len_hi2, len_hi3 ]  (u32 LE)
//! [ payload bytes … ]
//! ```
//!
//! The prefix applies to buffers the **guest returns**, and only to those.
//! Guest functions that return data return only the base pointer, and the host
//! reads the 4-byte prefix to know how many payload bytes follow.
//!
//! Buffers the **host passes in** are never prefixed. For `mag_init`,
//! `mag_step` and `mag_restore` alike the host calls `mag_alloc(payload_len)`,
//! writes the payload at `ptr + 0`, and passes `payload_len` as the second
//! argument. The length is a parameter, so a prefix would only duplicate it —
//! and that redundancy is exactly what let two drivers disagree about
//! `mag_restore`'s framing for as long as they did.
//!
//! The normative statement of all of this is `site/docs/sandbox-abi.md`.
//!
//! ## Versioning
//!
//! A conforming module exports `mag_abi_version() -> u32` and returns
//! [`MAG_ABI_VERSION`]. The host reads it at load time, before any payload is
//! exchanged, and refuses a module that declares anything else or omits the
//! export. Absence is a refusal, not a default — a module built against the
//! undeclared predecessor of this ABI has no version and is rejected rather
//! than having its bytes misinterpreted.
//!
//! ## ABI codec tests
//!
//! The tests in this module verify the encode/decode round-trip for the types
//! that cross the host↔guest boundary without requiring a live Wasm module.

use serde::{Deserialize, Serialize};

use magnetite_sdk::authority::{MatchConfig, RejectReason, StepOutput, Tick};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

/// The ABI version this host speaks, and the value a conforming module must
/// return from `mag_abi_version()`.
///
/// `1` is the first *declared* version. It is not "the first ABI" — it is the
/// first one that can be identified at load time. Modules built against the
/// undeclared predecessor return nothing at all and are refused.
pub const MAG_ABI_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Types that cross the boundary (host-side representations)
// ---------------------------------------------------------------------------

/// A single (player_id, input) frame inside a [`StepPayload`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFrame {
    pub player_id: u64,
    pub input: Input,
}

/// The complete `mag_step` payload: the authoritative tick, and the inputs for it.
///
/// The tick is carried explicitly so that the guest does not have to infer it by
/// counting calls. A guest that counts has a second source of truth for the same
/// number, and nothing can detect it drifting; a guest that is *told* the tick can
/// be checked, because it echoes back the tick it used in
/// [`GuestStepOutput::tick`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPayload {
    /// The authoritative tick this step advances to. Monotonic within a match.
    pub tick: Tick,
    /// Inputs for this tick, ordered by ascending `player_id`.
    pub inputs: Vec<InputFrame>,
}

/// The packed `StepOutput` that the guest writes into memory and returns
/// via `mag_step` → pointer.
///
/// The guest serialises this as JSON with the same field names; the host
/// deserialises it after reading the length-prefixed buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestStepOutput {
    /// Players whose inputs were rejected this tick.
    pub rejects: Vec<GuestReject>,
    /// FNV-1a 64-bit hash of game state after this tick.
    pub state_hash: u64,
    /// The tick the guest actually simulated.
    ///
    /// The host compares this against the tick it asked for and treats a
    /// mismatch as a failed step. This is the whole point of carrying the tick
    /// across the boundary: a guest that restored a snapshot without adopting
    /// its tick, or that is counting calls instead of reading the payload,
    /// reports the wrong number here and stops being silently wrong.
    pub tick: Tick,
}

/// A single rejection entry inside [`GuestStepOutput`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestReject {
    pub player_id: u64,
    pub reason: RejectReason,
}

impl From<GuestStepOutput> for StepOutput {
    fn from(g: GuestStepOutput) -> Self {
        StepOutput {
            rejects: g
                .rejects
                .into_iter()
                .map(|r| (PlayerId::new(r.player_id), r.reason))
                .collect(),
            state_hash: g.state_hash,
        }
    }
}

// ---------------------------------------------------------------------------
// Encode helpers (host → guest)
// ---------------------------------------------------------------------------

/// Serialise a [`MatchConfig`] to JSON bytes for `mag_init`.
pub fn encode_config(cfg: &MatchConfig) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(cfg)
}

/// Serialise the `mag_step` payload: the tick plus this tick's inputs.
pub fn encode_step_payload(
    tick: Tick,
    inputs: &[(PlayerId, Input)],
) -> Result<Vec<u8>, serde_json::Error> {
    let payload = StepPayload {
        tick,
        inputs: inputs
            .iter()
            .map(|(pid, inp)| InputFrame {
                player_id: pid.as_u64(),
                input: *inp,
            })
            .collect(),
    };
    serde_json::to_vec(&payload)
}

// ---------------------------------------------------------------------------
// Decode helpers (guest → host)
// ---------------------------------------------------------------------------

/// Decode a [`GuestStepOutput`] from raw JSON bytes returned by `mag_step`.
///
/// Returns the [`StepOutput`] and the tick the guest reported simulating. The
/// caller is expected to compare that tick against the one it asked for.
pub fn decode_step_output(bytes: &[u8]) -> Result<(StepOutput, Tick), serde_json::Error> {
    let guest: GuestStepOutput = serde_json::from_slice(bytes)?;
    let tick = guest.tick;
    Ok((guest.into(), tick))
}

/// Read and validate a length-prefixed buffer from a raw byte slice.
///
/// Expects the first 4 bytes to be a little-endian `u32` payload length,
/// followed by exactly that many payload bytes.
///
/// Returns the payload slice on success.
pub fn read_length_prefixed(buf: &[u8]) -> Result<&[u8], String> {
    if buf.len() < 4 {
        return Err(format!(
            "buffer too short for length prefix: {} bytes",
            buf.len()
        ));
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + len {
        return Err(format!(
            "buffer length prefix says {} bytes but only {} remain",
            len,
            buf.len() - 4
        ));
    }
    Ok(&buf[4..4 + len])
}

/// Build a length-prefixed buffer: 4-byte LE u32 length + payload.
///
/// The host does not use this on any outbound call — nothing it passes to the
/// guest is prefixed. It stays because it is the exact inverse of
/// [`read_length_prefixed`] and is what tests and reference implementations use
/// to construct the framing a *guest* is required to produce.
pub fn write_length_prefixed(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u32;
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::{MatchConfig, Topology};
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;

    // ---- Length-prefix codec -----------------------------------------------

    #[test]
    fn length_prefix_roundtrip_empty_payload() {
        let payload = b"";
        let framed = write_length_prefixed(payload);
        assert_eq!(framed.len(), 4);
        let decoded = read_length_prefixed(&framed).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn length_prefix_roundtrip_nonempty_payload() {
        let payload = b"hello, wasm!";
        let framed = write_length_prefixed(payload);
        assert_eq!(framed.len(), 4 + payload.len());
        let decoded = read_length_prefixed(&framed).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn length_prefix_roundtrip_binary_payload() {
        let payload: Vec<u8> = (0u8..=255).collect();
        let framed = write_length_prefixed(&payload);
        let decoded = read_length_prefixed(&framed).unwrap();
        assert_eq!(decoded, payload.as_slice());
    }

    #[test]
    fn read_length_prefix_rejects_too_short() {
        let bad = [1u8, 2, 3]; // only 3 bytes, not enough for prefix
        assert!(read_length_prefixed(&bad).is_err());
    }

    #[test]
    fn read_length_prefix_rejects_truncated_payload() {
        // Prefix says 10 bytes but only 3 follow.
        let mut buf = vec![10u8, 0, 0, 0];
        buf.extend_from_slice(b"abc"); // only 3 bytes
        assert!(read_length_prefixed(&buf).is_err());
    }

    // ---- encode_config -------------------------------------------------------

    #[test]
    fn encode_config_produces_valid_json() {
        let cfg = MatchConfig {
            topology: Topology::SingleRoom,
            max_players: 4,
            tick_hz: 60,
            seed: 12345,
            snapshot_every: 300,
        };
        let bytes = encode_config(&cfg).unwrap();
        // Must be valid JSON.
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["max_players"], 4);
        assert_eq!(val["tick_hz"], 60);
        assert_eq!(val["seed"], 12345);
    }

    #[test]
    fn encode_config_roundtrip() {
        let cfg = MatchConfig::auto(100);
        let bytes = encode_config(&cfg).unwrap();
        let decoded: MatchConfig = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.max_players, cfg.max_players);
        assert_eq!(decoded.tick_hz, cfg.tick_hz);
        assert_eq!(decoded.seed, cfg.seed);
        assert_eq!(decoded.snapshot_every, cfg.snapshot_every);
    }

    // ---- encode_step_payload -------------------------------------------------

    #[test]
    fn encode_step_payload_carries_the_tick_with_no_inputs() {
        let bytes = encode_step_payload(7, &[]).unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["tick"], 7);
        assert!(val["inputs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn encode_step_payload_single_player() {
        let p = PlayerId::new(7);
        let inp = Input {
            sequence: 42,
            ..Default::default()
        };
        let bytes = encode_step_payload(3, &[(p, inp)]).unwrap();
        let payload: StepPayload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload.tick, 3);
        assert_eq!(payload.inputs.len(), 1);
        assert_eq!(payload.inputs[0].player_id, 7);
        assert_eq!(payload.inputs[0].input.sequence, 42);
    }

    #[test]
    fn encode_step_payload_multiple_players() {
        let inputs: Vec<(PlayerId, Input)> = (1..=5)
            .map(|i| {
                (
                    PlayerId::new(i),
                    Input {
                        sequence: i * 10,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let bytes = encode_step_payload(99, &inputs).unwrap();
        let payload: StepPayload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload.tick, 99);
        assert_eq!(payload.inputs.len(), 5);
        for (i, frame) in payload.inputs.iter().enumerate() {
            assert_eq!(frame.player_id, (i + 1) as u64);
            assert_eq!(frame.input.sequence, (i + 1) as u64 * 10);
        }
    }

    #[test]
    fn step_payload_puts_tick_before_inputs() {
        // Not load-bearing for serde, which is order-insensitive, but a guest
        // parsing JSON by hand (see conformance/reference.wat) benefits from a
        // stable field order, and serde_json emits declaration order.
        let bytes = encode_step_payload(5, &[]).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.starts_with(r#"{"tick":5,"inputs":"#),
            "unexpected layout: {text}"
        );
    }

    // ---- decode_step_output --------------------------------------------------

    #[test]
    fn decode_step_output_no_rejects() {
        let json = r#"{"rejects":[],"state_hash":9876543210,"tick":12}"#;
        let (out, tick) = decode_step_output(json.as_bytes()).unwrap();
        assert!(out.rejects.is_empty());
        assert_eq!(out.state_hash, 9_876_543_210);
        assert_eq!(tick, 12);
    }

    #[test]
    fn decode_step_output_with_rejects() {
        let json = r#"{
            "rejects": [
                {"player_id": 3, "reason": "RateLimited"},
                {"player_id": 7, "reason": {"IllegalAction": "speed hack"}}
            ],
            "state_hash": 42,
            "tick": 1
        }"#;
        let (out, tick) = decode_step_output(json.as_bytes()).unwrap();
        assert_eq!(out.rejects.len(), 2);
        assert_eq!(out.rejects[0].0.as_u64(), 3);
        assert_eq!(out.rejects[0].1, RejectReason::RateLimited);
        assert_eq!(out.rejects[1].0.as_u64(), 7);
        assert_eq!(
            out.rejects[1].1,
            RejectReason::IllegalAction("speed hack".to_string())
        );
        assert_eq!(out.state_hash, 42);
        assert_eq!(tick, 1);
    }

    #[test]
    fn decode_step_output_invalid_json_returns_err() {
        let bad = b"not json at all";
        assert!(decode_step_output(bad).is_err());
    }

    #[test]
    fn decode_step_output_without_a_tick_is_rejected() {
        // A module that does not report its tick cannot be checked, so the field
        // is required rather than defaulted. Defaulting it to 0 would make every
        // pre-versioning module look like it agreed about tick 0.
        let json = r#"{"rejects":[],"state_hash":7}"#;
        assert!(decode_step_output(json.as_bytes()).is_err());
    }

    // ---- GuestStepOutput → StepOutput conversion ----------------------------

    #[test]
    fn guest_step_output_into_step_output() {
        let guest = GuestStepOutput {
            rejects: vec![GuestReject {
                player_id: 99,
                reason: RejectReason::StaleInput,
            }],
            state_hash: 0xDEAD_BEEF,
            tick: 4,
        };
        let out: StepOutput = guest.into();
        assert_eq!(out.state_hash, 0xDEAD_BEEF);
        assert_eq!(out.rejects.len(), 1);
        assert_eq!(out.rejects[0].0.as_u64(), 99);
        assert_eq!(out.rejects[0].1, RejectReason::StaleInput);
    }

    // ---- Tick type sanity ---------------------------------------------------

    #[test]
    fn tick_is_u64_alias() {
        use magnetite_sdk::authority::Tick;
        let t: Tick = u64::MAX;
        assert_eq!(t, u64::MAX);
    }
}
