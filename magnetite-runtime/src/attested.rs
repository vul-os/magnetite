//! Wire ingress for **client-attested** sensor input (seam §3.7).
//!
//! [`magnetite_seams::input::AttestedEventInput`] has existed as a Rust-side
//! queue with a plausibility gate in front of it, but nothing on the network
//! could reach it: `ClientNet` had no variant for an attested event and the
//! server had no route. A browser client could produce a correctly signed event
//! and there was nowhere to send it. This module is that route.
//!
//! # What this delivers, stated exactly
//!
//! It **delivers** attested input: a signed sensor claim from a client now
//! reaches the host, gets its authorship checked, gets screened for physical
//! implausibility, and lands in the attested queue with an ack or an explicit
//! refusal going back.
//!
//! It does **not** make that input *verifiable*, and no future version of this
//! file can. A cheater who never touched a camera can hand-write numbers well
//! inside human bounds, sign them with their own genuine key, and pass every
//! check here — `magnetite_seams::input`'s test
//! `a_plausible_synthetic_event_is_indistinguishable_from_a_real_one` pins that
//! ceiling in code precisely so it cannot be quietly forgotten. **Nothing in
//! this module is anti-cheat, verification, or security.** It is a delivery
//! path with a sanity screen on it.
//!
//! # The class boundary, enforced at the wire edge
//!
//! The reason `verify_replay` means anything is that the deterministic input
//! stream contains only replayable commands. An attested event admitted down
//! that path would leave replay verification still *passing* while no longer
//! *proving* anything — the failure mode is silent, which is the worst kind.
//!
//! So the two classes travel on separate frames ([`ClientNet::InputFrame`] vs
//! [`ClientNet::AttestedEvent`]), are handled by separate routes, land in
//! separate queues, and are answered on separate response variants. An attested
//! frame has no reachable code path to [`crate::connection::ConnectionManager`],
//! and `AttestedEventInput` itself refuses deterministic commands rather than
//! silently downgrading them. Three independent places would have to be wrong
//! at once for the classes to blur.
//!
//! # Fail-closed
//!
//! Every refusal below drops the event. Unsigned frames have no wire form at
//! all; malformed, wrongly-signed, implausible and flooding frames are refused
//! explicitly and the client is told which.

use std::sync::Mutex;

use magnetite_seams::SeamError;
use magnetite_seams::identity::RawKeypairAuth;
use magnetite_seams::input::{AttestedEventInput, PlausibilityLimits, SignedAttestedEvent};

/// Connection-level ceiling on **received** attested frames per second.
///
/// This is deliberately distinct from
/// [`PlausibilityLimits::max_events_per_sec`], which is a *per-player* screen on
/// events that already parsed and verified. This one is a *per-socket* screen on
/// frames that have not been verified yet, and it exists for a different reason:
/// signature verification costs real CPU, so a peer that floods garbage
/// signatures is a DoS vector regardless of whether any of them would ever have
/// been admitted. Bounding it before the verify call is the point.
///
/// Set above the plausibility rate so an honest client that trips its per-player
/// budget still gets a considered per-event answer rather than being cut off.
pub const MAX_ATTESTED_FRAMES_PER_SEC: u32 = 60;

/// Why an attested frame was refused. Carried back to the client in
/// [`magnetite_sdk::protocol::ServerNet::AttestedReject`].
///
/// **Every variant means "refused", none means "cheating proven".** A host may
/// drop these and may eventually disconnect a peer that produces a lot of them.
/// It holds no proof and should not claim one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestedRefusal {
    /// The frame did not parse as a signed attested event — including the case
    /// of an *unsigned* one, which has no wire representation by design.
    Malformed(String),
    /// The signature failed, or the event named a player other than the signing
    /// key. Either way authorship is not established, so there is nothing here
    /// worth screening.
    BadSignature,
    /// The claim is outside human-reachable bounds (rate, cooldown, velocity,
    /// confidence, timestamp sanity, replayed sequence number).
    Implausible(String),
    /// This connection sent attested frames faster than
    /// [`MAX_ATTESTED_FRAMES_PER_SEC`]. Refused *before* signature verification,
    /// so a flood cannot burn host CPU.
    RateLimited,
}

impl std::fmt::Display for AttestedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttestedRefusal::Malformed(why) => write!(f, "malformed attested frame: {why}"),
            AttestedRefusal::BadSignature => {
                write!(f, "signature does not verify for the named player")
            }
            AttestedRefusal::Implausible(why) => write!(f, "implausible: {why}"),
            AttestedRefusal::RateLimited => write!(
                f,
                "attested frames exceed {MAX_ATTESTED_FRAMES_PER_SEC}/s on this connection"
            ),
        }
    }
}

/// Per-connection ingress for [`magnetite_sdk::protocol::ClientNet::AttestedEvent`].
///
/// Owns one [`AttestedEventInput`] (queue + [`magnetite_seams::input::PlausibilityGate`])
/// and one connection-level frame-rate limiter. Per-connection rather than
/// shared, so one peer's flood cannot consume another peer's budget — the same
/// isolation property the gate maintains per player, applied a layer out.
///
/// Read the module docs before treating anything that comes out of here as
/// trustworthy.
pub struct AttestedIngress {
    input: AttestedEventInput,
    /// Receive timestamps (ms) of attested frames in the trailing second.
    recent_frames_ms: Mutex<Vec<u64>>,
    refused: Mutex<u64>,
}

impl Default for AttestedIngress {
    fn default() -> Self {
        Self::new(PlausibilityLimits::default())
    }
}

impl AttestedIngress {
    /// An ingress with explicit screening limits.
    pub fn new(limits: PlausibilityLimits) -> Self {
        Self {
            input: AttestedEventInput::new(limits),
            recent_frames_ms: Mutex::new(Vec::new()),
            refused: Mutex::new(0),
        }
    }

    /// The underlying attested queue — drain it to feed the sim.
    ///
    /// Note the type: this is an `InputClass::Attested` provider, and
    /// [`magnetite_seams::input::InputClass::is_replay_verifiable`] returns
    /// `false` for everything it yields. Check that before letting any of it
    /// settle an escrow or a ranking.
    pub fn input(&self) -> &AttestedEventInput {
        &self.input
    }

    /// How many frames this connection has had refused, for any reason.
    ///
    /// A signal to investigate a peer, not evidence against them.
    pub fn refused(&self) -> u64 {
        *self.refused.lock().unwrap()
    }

    /// Route one signed attested event at wall-clock `now_ms`.
    ///
    /// Order is load-bearing:
    ///
    /// 1. **Connection rate limit** — before verification, so flooding cannot be
    ///    turned into CPU burn.
    /// 2. **Signature** — establishes authorship. A frame that fails here never
    ///    reaches the gate, so it cannot touch another player's gate state.
    /// 3. **Plausibility gate**, then the queue — both inside
    ///    [`AttestedEventInput::submit_signed`], which advances gate state *only*
    ///    on acceptance. A rejected flood therefore cannot evict an honest
    ///    player's rate budget. That invariant lives in `input.rs` and is
    ///    preserved here by not reimplementing it.
    ///
    /// `recv_ms` is wall-clock receive time and drives **only** the connection
    /// frame-rate window. The plausibility gate reads its own clock inside
    /// [`AttestedEventInput::submit_signed`] and cannot be told a different one —
    /// which is correct for a host (a caller must not be able to hand the
    /// timestamp screen a convenient "now") but does mean a fixture with a dated
    /// `t_capture_ms` needs relaxed [`PlausibilityLimits`] to be admitted in a
    /// test.
    ///
    /// Returns the accepted `event.seq`, or the refusal to report back.
    pub async fn accept(
        &self,
        signed: &SignedAttestedEvent,
        recv_ms: u64,
    ) -> Result<u64, AttestedRefusal> {
        if let Err(e) = self.check_frame_rate(recv_ms) {
            return Err(self.refuse(e));
        }

        match self.input.submit_signed::<RawKeypairAuth>(signed).await {
            Ok(()) => Ok(signed.event.seq),
            Err(SeamError::Implausible(why)) => Err(self.refuse(AttestedRefusal::Implausible(why))),
            Err(SeamError::InvalidSignature) => Err(self.refuse(AttestedRefusal::BadSignature)),
            // `verify` returns Invalid when the event names a player other than
            // the signing key. Authorship is unestablished either way.
            Err(SeamError::Invalid(_)) => Err(self.refuse(AttestedRefusal::BadSignature)),
            Err(e) => Err(self.refuse(AttestedRefusal::Malformed(e.to_string()))),
        }
    }

    /// Refuse a frame that never parsed into a [`SignedAttestedEvent`].
    ///
    /// Still spends connection rate budget: a flood of unparseable frames is as
    /// much of a DoS as a flood of valid ones.
    pub fn refuse_malformed(&self, why: impl Into<String>, recv_ms: u64) -> AttestedRefusal {
        if let Err(e) = self.check_frame_rate(recv_ms) {
            return self.refuse(e);
        }
        self.refuse(AttestedRefusal::Malformed(why.into()))
    }

    fn refuse(&self, r: AttestedRefusal) -> AttestedRefusal {
        *self.refused.lock().unwrap() += 1;
        r
    }

    /// Trailing-second frame counter. Unlike the plausibility gate this *does*
    /// count refused frames — the cost it is defending against (parsing and
    /// verifying) was already paid by the time we knew.
    fn check_frame_rate(&self, recv_ms: u64) -> Result<(), AttestedRefusal> {
        let mut recent = self.recent_frames_ms.lock().unwrap();
        recent.retain(|t| recv_ms.saturating_sub(*t) < 1_000);
        if recent.len() as u32 >= MAX_ATTESTED_FRAMES_PER_SEC {
            return Err(AttestedRefusal::RateLimited);
        }
        recent.push(recv_ms);
        Ok(())
    }
}

/// Wall-clock unix milliseconds.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::identity::Identity;
    use magnetite_seams::input::InputProvider;
    use magnetite_seams::input::{AttestedEvent, InputEvent};

    /// The shared vector. This exact JSON is the fixture in wibbly's
    /// `packages/wibbly-magnetite/test/wire.test.ts`, and the `sig` was produced
    /// by `RawKeypairAuth::from_seed([7u8; 32])` over the same event. Pinning it
    /// on both sides is what makes "the two ends agree" a checked fact.
    const GOLDEN_FRAME: &str = r#"{"type":"attested_event","signed":{"event":{"player":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","kind":"swing","confidence":0.725,"vector":[0.125,-0.0625,0.0],"speed_mps":6.5,"t_capture_ms":1763000000123,"seq":42},"player_key":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","sig":"77bb88c4c43f147b5ff8749d9a22c6e275ae34564ed7bc1c4dc8bd5d28b05ef57f5c1ed0af1d2088c6e713bf01ab36c7a5112855e054a0c2bae11ae92f685e00"}}"#;

    /// `GOLDEN_FRAME`'s capture timestamp — the "now" its signature was made for.
    const GOLDEN_NOW_MS: u64 = 1_763_000_000_123;

    /// An ingress that will accept the *dated* golden fixture.
    ///
    /// The golden `t_capture_ms` is fixed (Nov 2025) so the client's signature
    /// stays reproducible, but the gate screens capture time against the real
    /// wall clock — by design, since a host must not let a caller choose "now".
    /// Only the age/skew bounds are relaxed; every other check stays at its
    /// default.
    fn golden_ingress() -> AttestedIngress {
        AttestedIngress::new(PlausibilityLimits {
            max_age_ms: u64::MAX,
            max_future_skew_ms: u64::MAX,
            ..Default::default()
        })
    }

    fn golden_signed() -> SignedAttestedEvent {
        match parse(GOLDEN_FRAME) {
            magnetite_sdk::protocol::ClientNet::AttestedEvent { signed } => *signed,
            other => panic!("golden frame is not an attested event: {other:?}"),
        }
    }

    fn parse(s: &str) -> magnetite_sdk::protocol::ClientNet {
        serde_json::from_str(s).expect("golden frame must deserialize")
    }

    fn signed_now(seed: u8, kind: &str, seq: u64, now_ms: u64) -> SignedAttestedEvent {
        let k = RawKeypairAuth::from_seed([seed; 32]);
        SignedAttestedEvent::sign(
            &k,
            AttestedEvent {
                player: k.pubkey(),
                kind: kind.into(),
                confidence: 0.9,
                vector: Some([1.0, 0.0, 0.0]),
                speed_mps: Some(6.0),
                t_capture_ms: now_ms,
                seq,
            },
        )
    }

    // ── The shared vector ───────────────────────────────────────────────────

    #[test]
    fn the_clients_golden_frame_parses_as_a_client_net_attested_event() {
        let signed = golden_signed();
        assert_eq!(
            signed.player_key.to_hex(),
            "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
        );
        assert_eq!(signed.event.kind, "swing");
        assert_eq!(signed.event.seq, 42);
        assert_eq!(signed.event.t_capture_ms, GOLDEN_NOW_MS);
        assert_eq!(signed.event.vector, Some([0.125, -0.0625, 0.0]));
        assert_eq!(signed.event.speed_mps, Some(6.5));
        // Signature produced by the client's fixture key verifies here.
        signed
            .verify::<RawKeypairAuth>()
            .expect("the client's golden signature must verify against this build");
    }

    /// Byte-for-byte, not structurally. `serde_json::to_value` would widen the
    /// `f32` `confidence` to `0.7250000238418579` and hide a real difference;
    /// `to_string` emits the `f32` shortest form the client actually sends.
    #[test]
    fn re_serializing_the_golden_frame_reproduces_the_clients_bytes() {
        let frame = parse(GOLDEN_FRAME);
        assert_eq!(
            serde_json::to_string(&frame).unwrap(),
            GOLDEN_FRAME,
            "the server's emitted bytes must equal the client's golden frame — \
             if this fails, wibbly's wire.ts must be updated in lockstep"
        );
    }

    // ── Admission ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn a_signed_valid_event_is_admitted() {
        let ing = golden_ingress();
        let seq = ing
            .accept(&golden_signed(), GOLDEN_NOW_MS)
            .await
            .expect("a correctly signed, plausible event is admitted");
        assert_eq!(seq, 42);

        let drained = ing.input().drain(GOLDEN_NOW_MS).await;
        assert_eq!(drained.len(), 1, "the event reached the attested queue");
        assert!(
            matches!(drained[0], InputEvent::Attested(_)),
            "and it arrived still labelled as attested"
        );
        assert_eq!(ing.refused(), 0);
    }

    #[tokio::test]
    async fn a_bad_signature_is_rejected() {
        let ing = golden_ingress();
        let mut tampered = golden_signed();
        // Same signature, faster claim. This is exactly the edit a relay would
        // make in flight.
        tampered.event.speed_mps = Some(12.0);
        assert_eq!(
            ing.accept(&tampered, GOLDEN_NOW_MS).await,
            Err(AttestedRefusal::BadSignature)
        );
        assert!(ing.input().drain(GOLDEN_NOW_MS).await.is_empty());
    }

    #[tokio::test]
    async fn an_event_signed_in_another_players_name_is_rejected() {
        let ing = AttestedIngress::default();
        let honest = RawKeypairAuth::from_seed([3u8; 32]);
        let attacker = RawKeypairAuth::from_seed([4u8; 32]);
        let mut ev = signed_now(4, "swing", 1, 10_000).event;
        ev.player = honest.pubkey();
        let forged = SignedAttestedEvent::sign(&attacker, ev);
        assert_eq!(
            ing.accept(&forged, 10_000).await,
            Err(AttestedRefusal::BadSignature)
        );
        assert!(ing.input().drain(0).await.is_empty());
    }

    #[tokio::test]
    async fn a_gate_failing_event_is_rejected_and_does_not_advance_gate_state() {
        let ing = AttestedIngress::default();
        let k = RawKeypairAuth::from_seed([9u8; 32]);
        let t = now_ms();

        // Superhuman speed — refused by the gate, correctly signed throughout.
        let mut fast = signed_now(9, "swing", 1, t).event;
        fast.speed_mps = Some(900.0);
        let fast = SignedAttestedEvent::sign(&k, fast);
        for _ in 0..10 {
            assert!(matches!(
                ing.accept(&fast, t).await,
                Err(AttestedRefusal::Implausible(_))
            ));
        }
        assert_eq!(ing.refused(), 10);
        assert!(ing.input().drain(t).await.is_empty());

        // The honest event that follows must be unaffected: not rate-starved by
        // the refusals, and not blocked by a sequence high-water mark the
        // refusals had no right to move.
        let good = signed_now(9, "swing", 1, t);
        ing.accept(&good, t)
            .await
            .expect("refusals must not have spent this player's budget or seq");
        assert_eq!(ing.input().drain(t).await.len(), 1);
    }

    #[tokio::test]
    async fn a_replayed_event_is_rejected() {
        let ing = AttestedIngress::default();
        let t = now_ms();
        let ev = signed_now(11, "swing", 7, t);
        ing.accept(&ev, t).await.unwrap();
        // Byte-identical resend: the signature is perfectly valid, which is
        // precisely why the sequence check has to catch it.
        assert!(matches!(
            ing.accept(&ev, t).await,
            Err(AttestedRefusal::Implausible(_))
        ));
        assert_eq!(ing.input().drain(t).await.len(), 1);
    }

    // ── The class boundary ──────────────────────────────────────────────────

    #[tokio::test]
    async fn an_attested_frame_cannot_enter_the_deterministic_path() {
        // 1. The frame decodes to the attested variant and to nothing else.
        //    There is no way to spell an attested event as an InputFrame.
        let frame = parse(GOLDEN_FRAME);
        assert!(
            matches!(
                frame,
                magnetite_sdk::protocol::ClientNet::AttestedEvent { .. }
            ),
            "an attested frame must never decode as InputFrame"
        );

        // 2. Everything the ingress yields is labelled attested, and the seam's
        //    own predicate refuses it the replay guarantee.
        let ing = golden_ingress();
        ing.accept(&golden_signed(), GOLDEN_NOW_MS).await.unwrap();
        for e in ing.input().drain(GOLDEN_NOW_MS).await {
            assert_eq!(e.class(), magnetite_seams::input::InputClass::Attested);
            assert!(
                !e.class().is_replay_verifiable(),
                "an attested event claiming replay verifiability would hollow out verify_replay"
            );
        }

        // 3. The reverse smuggle is refused too: a deterministic command offered
        //    to the attested provider is not silently downgraded.
        let det = InputEvent::Deterministic(magnetite_seams::input::DeterministicInput {
            player: RawKeypairAuth::from_seed([1u8; 32]).pubkey(),
            tick: 1,
            seq: 1,
            payload: b"jump".to_vec(),
        });
        assert!(ing.input().submit(det).await.is_err());
    }

    #[test]
    fn an_unsigned_attested_frame_has_no_wire_representation() {
        // wibbly's `AttestedFrameUnsigned` shape: `{type, event}` with no
        // `signed`. It carries no authorship binding at all, so it must not
        // decode — silence would be the client guessing, and acceptance would be
        // strictly worse than the signed form.
        let unsigned = r#"{"type":"attested_event","event":{"player":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","kind":"swing","confidence":0.725,"vector":null,"speed_mps":null,"t_capture_ms":1763000000123,"seq":42}}"#;
        assert!(
            serde_json::from_str::<magnetite_sdk::protocol::ClientNet>(unsigned).is_err(),
            "an unsigned attested frame must fail closed, not be admitted"
        );
    }

    // ── Denial of service ───────────────────────────────────────────────────

    #[tokio::test]
    async fn a_flood_is_rate_limited_at_the_connection() {
        let ing = AttestedIngress::default();
        let t = now_ms();
        // Every one of these is correctly signed and would otherwise only be
        // stopped per-player. The connection limit is what bounds the CPU.
        let mut refused_rate = 0;
        for seq in 1..500u64 {
            if let Err(AttestedRefusal::RateLimited) =
                ing.accept(&signed_now(12, "swing", seq, t), t).await
            {
                refused_rate += 1;
            }
        }
        assert!(
            refused_rate > 0,
            "an attested flood must be refused at the connection, not just per player"
        );
    }

    #[tokio::test]
    async fn the_rate_limiter_runs_before_signature_verification() {
        let ing = AttestedIngress::default();
        let t = now_ms();
        // Fill the connection budget with malformed frames, which never reach a
        // verify call at all.
        for _ in 0..MAX_ATTESTED_FRAMES_PER_SEC {
            ing.refuse_malformed("junk", t);
        }
        // A perfectly valid frame now hits the limiter first. That is the point:
        // the decision costs no signature verification.
        assert_eq!(
            ing.accept(&signed_now(13, "swing", 1, t), t).await,
            Err(AttestedRefusal::RateLimited)
        );
        // The window slides.
        ing.accept(&signed_now(13, "swing", 1, t + 1_500), t + 1_500)
            .await
            .expect("the limiter is a trailing window, not a ban");
    }

    #[test]
    fn one_connections_flood_does_not_spend_another_connections_budget() {
        let noisy = AttestedIngress::default();
        let quiet = AttestedIngress::default();
        let t = now_ms();
        for _ in 0..MAX_ATTESTED_FRAMES_PER_SEC * 2 {
            noisy.refuse_malformed("junk", t);
        }
        assert!(
            quiet.check_frame_rate(t).is_ok(),
            "ingress state is per-connection, so a flood is contained to its own socket"
        );
    }

    // ── Honesty ─────────────────────────────────────────────────────────────

    /// The ceiling, restated at the wire edge. `input.rs` pins this for the
    /// gate; this pins it for the route, so nobody reads "the server verifies
    /// attested events" into the fact that a route now exists.
    #[tokio::test]
    async fn a_plausible_synthetic_event_passes_the_whole_route() {
        let ing = AttestedIngress::default();
        let cheater = RawKeypairAuth::from_seed([5u8; 32]);
        let t = now_ms();
        // Never touched a camera. Hand-written numbers, well inside human range,
        // signed with a genuine key.
        let fabricated = SignedAttestedEvent::sign(
            &cheater,
            AttestedEvent {
                player: cheater.pubkey(),
                kind: "swing".into(),
                confidence: 0.97,
                vector: Some([0.0, 1.0, 0.0]),
                speed_mps: Some(7.5),
                t_capture_ms: t,
                seq: 1,
            },
        );
        ing.accept(&fabricated, t).await.expect(
            "synthetic-but-plausible input is admitted — no check in this route catches it, \
             and none can. This route delivers attested input; it does not verify it.",
        );
    }
}
