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
//! The host writes payload bytes at `ptr + 4` where `ptr` is obtained from
//! `mag_alloc(4 + payload_len)`.  Guest functions that return data return only
//! the base pointer; the host reads the 4-byte prefix to know how many payload
//! bytes follow.
//!
//! ## ABI codec tests
//!
//! The tests in this module verify the encode/decode round-trip for the types
//! that cross the host↔guest boundary without requiring a live Wasm module.

use serde::{Deserialize, Serialize};

use magnetite_sdk::authority::{MatchConfig, RejectReason, StepOutput};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

// ---------------------------------------------------------------------------
// Types that cross the boundary (host-side representations)
// ---------------------------------------------------------------------------

/// A single (player_id, input) frame as sent to `mag_step`.
///
/// JSON-serialised and written into guest memory as a JSON array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFrame {
    pub player_id: u64,
    pub input: Input,
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

/// Serialise a slice of `(PlayerId, Input)` pairs to JSON bytes for `mag_step`.
///
/// The guest receives a JSON array of [`InputFrame`] objects.
pub fn encode_inputs(inputs: &[(PlayerId, Input)]) -> Result<Vec<u8>, serde_json::Error> {
    let frames: Vec<InputFrame> = inputs
        .iter()
        .map(|(pid, inp)| InputFrame {
            player_id: pid.as_u64(),
            input: inp.clone(),
        })
        .collect();
    serde_json::to_vec(&frames)
}

// ---------------------------------------------------------------------------
// Decode helpers (guest → host)
// ---------------------------------------------------------------------------

/// Decode a [`GuestStepOutput`] from raw JSON bytes returned by `mag_step`.
pub fn decode_step_output(bytes: &[u8]) -> Result<StepOutput, serde_json::Error> {
    let guest: GuestStepOutput = serde_json::from_slice(bytes)?;
    Ok(guest.into())
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

    // ---- encode_inputs -------------------------------------------------------

    #[test]
    fn encode_inputs_empty_slice() {
        let bytes = encode_inputs(&[]).unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(val.as_array().unwrap().is_empty());
    }

    #[test]
    fn encode_inputs_single_player() {
        let p = PlayerId::new(7);
        let inp = Input {
            sequence: 42,
            ..Default::default()
        };
        let bytes = encode_inputs(&[(p, inp)]).unwrap();
        let frames: Vec<InputFrame> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].player_id, 7);
        assert_eq!(frames[0].input.sequence, 42);
    }

    #[test]
    fn encode_inputs_multiple_players() {
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
        let bytes = encode_inputs(&inputs).unwrap();
        let frames: Vec<InputFrame> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(frames.len(), 5);
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.player_id, (i + 1) as u64);
            assert_eq!(frame.input.sequence, (i + 1) as u64 * 10);
        }
    }

    // ---- decode_step_output --------------------------------------------------

    #[test]
    fn decode_step_output_no_rejects() {
        let json = r#"{"rejects":[],"state_hash":9876543210}"#;
        let out = decode_step_output(json.as_bytes()).unwrap();
        assert!(out.rejects.is_empty());
        assert_eq!(out.state_hash, 9_876_543_210);
    }

    #[test]
    fn decode_step_output_with_rejects() {
        let json = r#"{
            "rejects": [
                {"player_id": 3, "reason": "RateLimited"},
                {"player_id": 7, "reason": {"IllegalAction": "speed hack"}}
            ],
            "state_hash": 42
        }"#;
        let out = decode_step_output(json.as_bytes()).unwrap();
        assert_eq!(out.rejects.len(), 2);
        assert_eq!(out.rejects[0].0.as_u64(), 3);
        assert_eq!(out.rejects[0].1, RejectReason::RateLimited);
        assert_eq!(out.rejects[1].0.as_u64(), 7);
        assert_eq!(
            out.rejects[1].1,
            RejectReason::IllegalAction("speed hack".to_string())
        );
        assert_eq!(out.state_hash, 42);
    }

    #[test]
    fn decode_step_output_invalid_json_returns_err() {
        let bad = b"not json at all";
        assert!(decode_step_output(bad).is_err());
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
