//! Seam §3.7 — `InputProvider` (where a player's input comes from).
//!
//! # The whole point of this module is the boundary it draws
//!
//! Magnetite's moat is deterministic authoritative simulation: clients send
//! *inputs*, the host steps the sim, every tick lands in a `ReplayLog`, and
//! anybody can re-run `verify_replay` and **prove** tampering from the record
//! alone. That property holds only because the inputs are deterministic —
//! replaying the same ordered commands from the same seed reproduces the same
//! state, so a divergence *is* evidence.
//!
//! Not every input source can offer that. A camera-gesture stream (the reason
//! this seam exists — see `wibbly/WIBBLY.md` §6) is a **nondeterministic sensor
//! reading**. There is no way to re-derive "the player swung at 6.2 m/s" from a
//! log, because the pixels that produced it are gone and were never authoritative
//! in the first place. Such input is **client-attested**: the client asserts what
//! happened and the host decides whether to believe it.
//!
//! So this seam splits input into two classes and refuses to let them blur:
//!
//! | Class | Example | Replay-verifiable | What the host can prove |
//! |---|---|---|---|
//! | [`InputClass::Deterministic`] | keyboard, gamepad, scripted bot | **yes** | tampering, from the log alone |
//! | [`InputClass::Attested`] | camera gesture, IMU, any sensor | **no** | only *implausibility*, never intent |
//!
//! ## Read this before trusting an attested event
//!
//! * A [`SignedAttestedEvent`] signature proves **authorship**, not **truth**.
//!   It binds "this key sent this event" — it says nothing about whether a human
//!   body actually moved. A cheater signing synthetic events with their own real
//!   key produces a perfectly valid signature.
//! * [`PlausibilityGate`] rejects events that are outside human-reachable bounds
//!   (rate, cooldown, velocity, confidence, timestamp sanity, replayed sequence
//!   numbers). Rejection means "this is not physically reachable", and
//!   **acceptance means nothing stronger than "not obviously impossible"**. A
//!   cheater who synthesises *plausible* events is not detectable here, and
//!   `verify_replay` cannot help — that is a property of sensor input, not a bug
//!   to be fixed later.
//! * Consequently a host must never settle a wager escrow (§3.6) or issue a
//!   competitive ranking from attested input on the strength of replay proof.
//!   [`InputClass::is_replay_verifiable`] exists so that decision can be made in
//!   code rather than by a reader remembering this paragraph.
//!
//! ## What is actually built here
//!
//! Traits, the two event classes, and the host-side plausibility gate — plus two
//! working offline providers:
//!
//! * [`LocalDeviceInput`] — **the default**. A deterministic keyboard/gamepad
//!   style queue. Depends on no camera, no model, and no external crate. It
//!   *refuses* attested events at runtime, so the class boundary is enforced and
//!   not merely documented.
//! * [`AttestedEventInput`] — the host-side ingress for attested events, gated by
//!   [`PlausibilityGate`]. It is transport-agnostic and contains **no camera
//!   capture, no pose model, and no vendor code of any kind**. Nothing in this
//!   repo produces gesture events today; wibbly is not a dependency of magnetite
//!   and no camera integration exists here. This is the socket wibbly will plug
//!   into, and only that.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{Result, SeamError};
use crate::identity::{Identity, PubKey, Sig};

/// Simulation tick number (matches the authoritative sim's tick counter).
pub type Tick = u64;

/// The guarantee an input stream carries. **This is the load-bearing type in
/// this module** — it is what stops attested input being mistaken for
/// deterministic input somewhere downstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputClass {
    /// Discrete, reproducible commands (keyboard, gamepad, scripted bot).
    ///
    /// Replaying these against the same `(state, tick, seed)` reproduces the
    /// same result, so `ReplayLog` / `verify_replay` can prove tampering.
    Deterministic,
    /// Sensor-derived assertions from a client (camera gesture, IMU, …).
    ///
    /// **Not replay-verifiable at any point, ever.** The host simulates
    /// authoritatively over what it receives and can screen for implausibility,
    /// but it cannot re-derive the event and therefore cannot prove intent.
    Attested,
}

impl InputClass {
    /// Whether events of this class can be proven with `verify_replay`.
    ///
    /// Call this instead of matching on the variant when the decision is
    /// "may I treat this as evidence?" — e.g. before settling a wager escrow.
    pub fn is_replay_verifiable(&self) -> bool {
        matches!(self, InputClass::Deterministic)
    }
}

/// A deterministic, replay-verifiable input command.
///
/// The payload is opaque bytes: this crate never interprets game commands, it
/// only carries them. Ordering is by `(tick, seq)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicInput {
    /// Whose input this is.
    pub player: PubKey,
    /// The tick the command applies to.
    pub tick: Tick,
    /// Per-player monotonic sequence number (orders commands within a tick).
    pub seq: u64,
    /// Opaque, game-defined command bytes.
    pub payload: Vec<u8>,
}

/// A **client-attested** sensor event. Not replay-verifiable — see the module docs.
///
/// Every field is a *claim made by the client*. `confidence`, `speed_mps` and
/// `t_capture_ms` are reported by the sending device and cannot be checked
/// against ground truth by anyone; they exist so [`PlausibilityGate`] can screen
/// for the physically unreachable, not so the host can verify what happened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttestedEvent {
    /// Whose event this claims to be.
    pub player: PubKey,
    /// Game-defined event kind (`"swing"`, `"punch"`, `"pinch"`, …).
    pub kind: String,
    /// Client-reported recognizer confidence in `0.0..=1.0`. **Client-supplied**
    /// — a cheater picks their own value, so a high confidence is not evidence.
    pub confidence: f32,
    /// Optional direction/magnitude hint in game units.
    pub vector: Option<[f32; 3]>,
    /// Optional client-reported peak speed in metres/second. Screened against
    /// [`PlausibilityLimits::max_speed_mps`].
    pub speed_mps: Option<f32>,
    /// Client's **capture** timestamp in unix milliseconds (not detection time —
    /// recognition latency would otherwise be charged to the player).
    pub t_capture_ms: u64,
    /// Per-player monotonic sequence number. The gate refuses non-increasing
    /// values, so a captured event cannot simply be re-sent.
    pub seq: u64,
}

/// Domain-separation tag for attested-event signatures (`v1`).
pub const ATTESTED_DOMAIN: &[u8] = b"magnetite/input/attested/v1";

fn push_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
    buf.extend_from_slice(b);
}

impl AttestedEvent {
    /// Canonical, serialization-independent bytes for this event.
    ///
    /// Built field-by-field (never from JSON) so two peers on different serde
    /// versions still agree byte-for-byte on what was signed. Floats are covered
    /// by their IEEE-754 bit pattern so the bytes are exact.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(128);
        b.extend_from_slice(&self.player.0);
        push_bytes(&mut b, self.kind.as_bytes());
        b.extend_from_slice(&self.confidence.to_bits().to_le_bytes());
        match &self.vector {
            Some(v) => {
                b.push(1);
                for c in v {
                    b.extend_from_slice(&c.to_bits().to_le_bytes());
                }
            }
            None => b.push(0),
        }
        match &self.speed_mps {
            Some(s) => {
                b.push(1);
                b.extend_from_slice(&s.to_bits().to_le_bytes());
            }
            None => b.push(0),
        }
        b.extend_from_slice(&self.t_capture_ms.to_le_bytes());
        b.extend_from_slice(&self.seq.to_le_bytes());
        b
    }
}

/// An [`AttestedEvent`] signed by the claiming player's key.
///
/// **A valid signature here proves authorship and nothing else.** It stops one
/// player forging events *in another player's name*, and stops a relay editing
/// events in flight. It does **not** make the sensor reading true: a cheater
/// synthesising events signs them with their own genuine key and passes
/// verification every time. Treat this as attribution, never as attestation of
/// physical reality — the name of the class is [`InputClass::Attested`] because
/// the *client* attests, not because anyone verified.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedAttestedEvent {
    /// The event being claimed.
    pub event: AttestedEvent,
    /// The claiming player's public key (must equal `event.player`).
    pub player_key: PubKey,
    /// `player_key`'s signature over [`SignedAttestedEvent::signing_bytes`].
    pub sig: Sig,
}

impl SignedAttestedEvent {
    /// Canonical bytes covered by [`SignedAttestedEvent::sig`].
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::payload(&self.event, &self.player_key)
    }

    fn payload(event: &AttestedEvent, player_key: &PubKey) -> Vec<u8> {
        let mut b = Vec::with_capacity(192);
        b.extend_from_slice(ATTESTED_DOMAIN);
        b.extend_from_slice(&event.signing_bytes());
        b.extend_from_slice(&player_key.0);
        b
    }

    /// Sign an event with the claiming player's identity.
    pub fn sign<I: Identity>(id: &I, event: AttestedEvent) -> Self {
        let player_key = id.pubkey();
        let sig = id.sign(&Self::payload(&event, &player_key));
        Self {
            event,
            player_key,
            sig,
        }
    }

    /// Verify authorship. **Fails closed**: an event whose signature is bad, or
    /// which is signed by a key other than the one it names as the player, is
    /// refused.
    ///
    /// Passing this check means "this key sent this" — re-read the type docs
    /// before reading anything more into it.
    pub fn verify<I: Identity>(&self) -> Result<()> {
        if self.player_key != self.event.player {
            return Err(SeamError::Invalid(
                "attested event names a different player than the signing key".into(),
            ));
        }
        if !I::verify(&self.player_key, &self.signing_bytes(), &self.sig) {
            return Err(SeamError::InvalidSignature);
        }
        Ok(())
    }
}

/// One input event of either class.
///
/// Deliberately an enum rather than a shared struct with a flag: a consumer
/// cannot read the payload without first matching on the class, so "which
/// guarantee does this carry?" is a question the compiler makes you answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Replay-verifiable command.
    Deterministic(DeterministicInput),
    /// Client-attested sensor event — see [`InputClass::Attested`].
    Attested(AttestedEvent),
}

impl InputEvent {
    /// The guarantee class of this event.
    pub fn class(&self) -> InputClass {
        match self {
            InputEvent::Deterministic(_) => InputClass::Deterministic,
            InputEvent::Attested(_) => InputClass::Attested,
        }
    }
    /// The player this event is attributed to.
    pub fn player(&self) -> &PubKey {
        match self {
            InputEvent::Deterministic(d) => &d.player,
            InputEvent::Attested(a) => &a.player,
        }
    }
}

// ---------------------------------------------------------------------------
// Plausibility screening (attested input only)
// ---------------------------------------------------------------------------

/// Why an attested event was refused.
///
/// Every variant means **"not physically reachable"**. None of them mean
/// "cheating proven" — a host may drop the event, and may rate-limit or
/// eventually disconnect a peer that produces a lot of them, but it holds no
/// proof and should not claim one.
#[derive(Clone, Debug, PartialEq)]
pub enum Implausible {
    /// More events in the last second than a human can produce.
    RateExceeded {
        /// Events observed in the trailing window.
        observed: u32,
        /// Configured ceiling.
        limit: u32,
    },
    /// This kind fired again before its cooldown elapsed.
    Cooldown {
        /// Milliseconds since the last accepted event of this kind.
        elapsed_ms: u64,
        /// Configured cooldown.
        cooldown_ms: u64,
    },
    /// Reported speed exceeds what a human limb can reach.
    SpeedUnreachable {
        /// Client-reported speed.
        reported_mps: f32,
        /// Configured ceiling.
        limit_mps: f32,
    },
    /// Confidence below the floor, or not a number in `0.0..=1.0`.
    ImplausibleConfidence,
    /// Capture timestamp is too far in the past to still be actionable.
    StaleTimestamp,
    /// Capture timestamp is in the future beyond the tolerated clock skew.
    FutureTimestamp,
    /// Sequence number did not advance — a replayed or reordered event.
    SequenceReplayed,
    /// The event kind is not one this host accepts.
    UnknownKind,
}

impl std::fmt::Display for Implausible {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Implausible::RateExceeded { observed, limit } => {
                write!(f, "rate {observed}/s exceeds limit {limit}/s")
            }
            Implausible::Cooldown {
                elapsed_ms,
                cooldown_ms,
            } => write!(f, "fired {elapsed_ms}ms into a {cooldown_ms}ms cooldown"),
            Implausible::SpeedUnreachable {
                reported_mps,
                limit_mps,
            } => write!(f, "speed {reported_mps} m/s exceeds limit {limit_mps} m/s"),
            Implausible::ImplausibleConfidence => write!(f, "confidence out of range or too low"),
            Implausible::StaleTimestamp => write!(f, "capture timestamp is stale"),
            Implausible::FutureTimestamp => write!(f, "capture timestamp is in the future"),
            Implausible::SequenceReplayed => write!(f, "sequence number did not advance"),
            Implausible::UnknownKind => write!(f, "unaccepted event kind"),
        }
    }
}

/// Human-reachability bounds a host screens attested events against.
///
/// The defaults are deliberately generous — a gate that rejects real players is
/// worse than one that admits a careful cheater, because the cheater was never
/// preventable here in the first place. Tune per game.
#[derive(Clone, Debug, PartialEq)]
pub struct PlausibilityLimits {
    /// Maximum accepted events per player per trailing second.
    pub max_events_per_sec: u32,
    /// Minimum gap between two accepted events *of the same kind*.
    pub cooldown_ms: u64,
    /// Fastest limb speed a human is assumed to reach, in metres/second.
    /// ~20 m/s covers a hard tennis-racket-hand swing with margin.
    pub max_speed_mps: f32,
    /// Confidence floor; events below this are dropped as noise.
    pub min_confidence: f32,
    /// Tolerated forward clock skew on `t_capture_ms`.
    pub max_future_skew_ms: u64,
    /// Oldest capture timestamp still actionable.
    pub max_age_ms: u64,
    /// If non-empty, only these event kinds are accepted.
    pub accepted_kinds: Vec<String>,
}

impl Default for PlausibilityLimits {
    fn default() -> Self {
        Self {
            max_events_per_sec: 20,
            cooldown_ms: 100,
            max_speed_mps: 20.0,
            min_confidence: 0.35,
            max_future_skew_ms: 2_000,
            max_age_ms: 5_000,
            accepted_kinds: Vec::new(),
        }
    }
}

#[derive(Default)]
struct PlayerWindow {
    /// Accept timestamps in the trailing rate window.
    recent_ms: Vec<u64>,
    /// Last accepted `t_capture_ms` per event kind (cooldown tracking).
    last_kind_ms: HashMap<String, u64>,
    /// Highest sequence number accepted so far.
    high_seq: Option<u64>,
}

/// Host-side screening for [`InputClass::Attested`] events.
///
/// What it does: enforces rate limits, per-kind cooldowns, human-reachable
/// velocity, a confidence floor, timestamp sanity, and monotonic sequence
/// numbers, per player.
///
/// **What it does not do:** prove anything. Acceptance is not a verdict of
/// honesty; it only means the claim was not physically impossible. This is the
/// strongest check available for sensor input and is deliberately weaker than
/// `verify_replay`, which does not apply to this class at all.
pub struct PlausibilityGate {
    limits: PlausibilityLimits,
    players: Mutex<HashMap<PubKey, PlayerWindow>>,
}

impl Default for PlausibilityGate {
    fn default() -> Self {
        Self::new(PlausibilityLimits::default())
    }
}

impl PlausibilityGate {
    /// A gate with explicit limits.
    pub fn new(limits: PlausibilityLimits) -> Self {
        Self {
            limits,
            players: Mutex::new(HashMap::new()),
        }
    }

    /// The limits in force.
    pub fn limits(&self) -> &PlausibilityLimits {
        &self.limits
    }

    /// Screen one event at wall-clock `now_ms`.
    ///
    /// State (rate window, cooldowns, sequence high-water mark) advances **only
    /// on acceptance**, so a burst of rejected events cannot be used to push a
    /// competitor's own legitimate events out of the window.
    pub fn admit(
        &self,
        ev: &AttestedEvent,
        now_ms: u64,
    ) -> std::result::Result<(), Implausible> {
        if !self.limits.accepted_kinds.is_empty()
            && !self.limits.accepted_kinds.iter().any(|k| k == &ev.kind)
        {
            return Err(Implausible::UnknownKind);
        }
        if !ev.confidence.is_finite()
            || !(0.0..=1.0).contains(&ev.confidence)
            || ev.confidence < self.limits.min_confidence
        {
            return Err(Implausible::ImplausibleConfidence);
        }
        if ev.t_capture_ms > now_ms.saturating_add(self.limits.max_future_skew_ms) {
            return Err(Implausible::FutureTimestamp);
        }
        if now_ms.saturating_sub(ev.t_capture_ms) > self.limits.max_age_ms {
            return Err(Implausible::StaleTimestamp);
        }
        if let Some(s) = ev.speed_mps {
            if !s.is_finite() || s > self.limits.max_speed_mps {
                return Err(Implausible::SpeedUnreachable {
                    reported_mps: s,
                    limit_mps: self.limits.max_speed_mps,
                });
            }
        }

        let mut players = self.players.lock().unwrap();
        let w = players.entry(ev.player).or_default();

        if let Some(high) = w.high_seq {
            if ev.seq <= high {
                return Err(Implausible::SequenceReplayed);
            }
        }
        if let Some(&last) = w.last_kind_ms.get(&ev.kind) {
            let elapsed = ev.t_capture_ms.saturating_sub(last);
            if elapsed < self.limits.cooldown_ms {
                return Err(Implausible::Cooldown {
                    elapsed_ms: elapsed,
                    cooldown_ms: self.limits.cooldown_ms,
                });
            }
        }
        w.recent_ms.retain(|t| now_ms.saturating_sub(*t) < 1_000);
        if w.recent_ms.len() as u32 >= self.limits.max_events_per_sec {
            return Err(Implausible::RateExceeded {
                observed: w.recent_ms.len() as u32 + 1,
                limit: self.limits.max_events_per_sec,
            });
        }

        // Accepted — and only now does any state move.
        w.recent_ms.push(now_ms);
        w.last_kind_ms.insert(ev.kind.clone(), ev.t_capture_ms);
        w.high_seq = Some(ev.seq);
        Ok(())
    }

    /// Forget a player's screening state (they left the session).
    pub fn forget(&self, player: &PubKey) {
        self.players.lock().unwrap().remove(player);
    }
}

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// The input seam (§3.7).
///
/// A provider is the queue between *some* input source and the authoritative
/// simulation. The runtime programs against this trait and never names a device,
/// a pose model, or a vendor.
///
/// Implementors **must** report a truthful [`InputProvider::class`] and must
/// reject events of the other class from [`InputProvider::submit`]. That is the
/// enforcement point for the guarantee boundary: a provider that quietly accepts
/// attested events while advertising `Deterministic` would silently break
/// `verify_replay`'s meaning downstream.
#[async_trait::async_trait]
pub trait InputProvider {
    /// The guarantee class of every event this provider emits.
    ///
    /// If this is [`InputClass::Deterministic`], the emitted stream is safe to
    /// feed a `ReplayLog` as evidence. If it is [`InputClass::Attested`], it is
    /// **not**, and no amount of later processing makes it so.
    fn class(&self) -> InputClass;

    /// Offer an event to this provider (local device loop, or a host receiving
    /// from a remote client).
    ///
    /// Fails closed on a class mismatch, and — for attested providers — on
    /// failing plausibility screening.
    async fn submit(&self, event: InputEvent) -> Result<()>;

    /// Drain everything ready to be stepped at `now_ms`, in submission order.
    async fn drain(&self, now_ms: u64) -> Vec<InputEvent>;

    /// Screening limits, if this provider screens at all.
    ///
    /// `None` for deterministic providers: there is nothing to screen because
    /// the replay log is a strictly stronger check.
    fn plausibility_limits(&self) -> Option<&PlausibilityLimits> {
        None
    }
}

/// **The default provider.** A deterministic keyboard/gamepad-style input queue.
///
/// Offline, dependency-free, and replay-verifiable: this is what
/// `magnetite dev` uses, and it is why nothing in magnetite depends on a camera,
/// a pose model, or wibbly.
///
/// It refuses [`InputEvent::Attested`] outright — the class boundary is a
/// runtime error here, not a comment.
#[derive(Default)]
pub struct LocalDeviceInput {
    queue: Mutex<Vec<InputEvent>>,
}

impl LocalDeviceInput {
    /// An empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience for the common case: queue a raw command for `player`.
    pub fn press(&self, player: PubKey, tick: Tick, seq: u64, payload: impl Into<Vec<u8>>) {
        self.queue
            .lock()
            .unwrap()
            .push(InputEvent::Deterministic(DeterministicInput {
                player,
                tick,
                seq,
                payload: payload.into(),
            }));
    }
}

#[async_trait::async_trait]
impl InputProvider for LocalDeviceInput {
    fn class(&self) -> InputClass {
        InputClass::Deterministic
    }

    async fn submit(&self, event: InputEvent) -> Result<()> {
        match event {
            InputEvent::Deterministic(_) => {
                self.queue.lock().unwrap().push(event);
                Ok(())
            }
            // Not a nicety. Accepting this would put unverifiable events into a
            // stream the rest of the system believes it can prove.
            InputEvent::Attested(_) => Err(SeamError::Invalid(
                "LocalDeviceInput is a deterministic provider and cannot carry attested \
                 sensor events; use an InputClass::Attested provider"
                    .into(),
            )),
        }
    }

    async fn drain(&self, _now_ms: u64) -> Vec<InputEvent> {
        std::mem::take(&mut *self.queue.lock().unwrap())
    }
}

/// Host-side ingress for [`InputClass::Attested`] events, screened by a
/// [`PlausibilityGate`].
///
/// **Scope, stated plainly:** this is a queue plus a screening gate. It does not
/// capture a camera, run a pose model, or recognise a gesture, and magnetite
/// contains no code that does. Producing [`AttestedEvent`]s is the client's job
/// (wibbly's, in the case this seam was designed for); wibbly is not a
/// dependency and no such client is wired up in this repo today.
///
/// Events that fail screening are dropped and counted ([`Self::rejected`]) so an
/// operator can see the rate. A high rejection count is a signal to investigate,
/// **not** proof of cheating — see [`Implausible`].
pub struct AttestedEventInput {
    gate: PlausibilityGate,
    queue: Mutex<Vec<InputEvent>>,
    rejected: Mutex<u64>,
}

impl Default for AttestedEventInput {
    fn default() -> Self {
        Self::new(PlausibilityLimits::default())
    }
}

impl AttestedEventInput {
    /// An ingress with explicit screening limits.
    pub fn new(limits: PlausibilityLimits) -> Self {
        Self {
            gate: PlausibilityGate::new(limits),
            queue: Mutex::new(Vec::new()),
            rejected: Mutex::new(0),
        }
    }

    /// How many events have been dropped as implausible.
    pub fn rejected(&self) -> u64 {
        *self.rejected.lock().unwrap()
    }

    /// The screening gate, for hosts that want to inspect or reset state.
    pub fn gate(&self) -> &PlausibilityGate {
        &self.gate
    }

    /// Verify authorship **and** screen plausibility in one step — the path a
    /// host should use for events arriving off the wire.
    ///
    /// Both checks together still amount to "sent by this key, and not
    /// physically impossible". That is the ceiling for this input class.
    pub async fn submit_signed<I: Identity>(&self, signed: &SignedAttestedEvent) -> Result<()> {
        signed.verify::<I>()?;
        self.submit(InputEvent::Attested(signed.event.clone())).await
    }
}

#[async_trait::async_trait]
impl InputProvider for AttestedEventInput {
    fn class(&self) -> InputClass {
        InputClass::Attested
    }

    async fn submit(&self, event: InputEvent) -> Result<()> {
        let ev = match &event {
            InputEvent::Attested(a) => a,
            // A deterministic command routed through the attested provider would
            // be *downgraded* — it would lose the replay guarantee it actually
            // had. Refuse rather than silently weaken it.
            InputEvent::Deterministic(_) => {
                return Err(SeamError::Invalid(
                    "AttestedEventInput carries sensor events only; deterministic commands \
                     belong on a deterministic provider so they keep replay verifiability"
                        .into(),
                ))
            }
        };
        let now_ms = crate::now_unix().saturating_mul(1_000).max(ev.t_capture_ms);
        if let Err(why) = self.gate.admit(ev, now_ms) {
            *self.rejected.lock().unwrap() += 1;
            return Err(SeamError::Implausible(why.to_string()));
        }
        self.queue.lock().unwrap().push(event);
        Ok(())
    }

    async fn drain(&self, _now_ms: u64) -> Vec<InputEvent> {
        std::mem::take(&mut *self.queue.lock().unwrap())
    }

    fn plausibility_limits(&self) -> Option<&PlausibilityLimits> {
        Some(self.gate.limits())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::RawKeypairAuth;

    fn pk(b: u8) -> PubKey {
        PubKey([b; 32])
    }

    fn ev(player: PubKey, kind: &str, seq: u64, t: u64) -> AttestedEvent {
        AttestedEvent {
            player,
            kind: kind.into(),
            confidence: 0.9,
            vector: Some([1.0, 0.0, 0.0]),
            speed_mps: Some(6.0),
            t_capture_ms: t,
            seq,
        }
    }

    // ── The class boundary ──────────────────────────────────────────────────

    #[test]
    fn only_deterministic_input_claims_replay_verifiability() {
        assert!(InputClass::Deterministic.is_replay_verifiable());
        assert!(
            !InputClass::Attested.is_replay_verifiable(),
            "attested sensor input must never claim the replay guarantee"
        );
    }

    #[tokio::test]
    async fn deterministic_provider_refuses_attested_events() {
        let p = LocalDeviceInput::new();
        assert_eq!(p.class(), InputClass::Deterministic);
        let err = p
            .submit(InputEvent::Attested(ev(pk(1), "swing", 1, 1_000)))
            .await
            .expect_err("the boundary must be enforced, not just documented");
        assert!(matches!(err, SeamError::Invalid(_)));
        assert!(p.drain(0).await.is_empty(), "nothing leaked into the queue");
    }

    #[tokio::test]
    async fn attested_provider_refuses_to_downgrade_deterministic_commands() {
        let p = AttestedEventInput::default();
        assert_eq!(p.class(), InputClass::Attested);
        let cmd = InputEvent::Deterministic(DeterministicInput {
            player: pk(1),
            tick: 3,
            seq: 1,
            payload: b"jump".to_vec(),
        });
        assert!(p.submit(cmd).await.is_err());
    }

    #[tokio::test]
    async fn deterministic_provider_queues_and_drains_in_order() {
        let p = LocalDeviceInput::new();
        p.press(pk(1), 1, 1, b"left".to_vec());
        p.press(pk(1), 1, 2, b"right".to_vec());
        let drained = p.drain(0).await;
        assert_eq!(drained.len(), 2);
        assert!(drained.iter().all(|e| e.class().is_replay_verifiable()));
        assert!(p.drain(0).await.is_empty(), "drain empties the queue");
        assert!(
            p.plausibility_limits().is_none(),
            "a deterministic provider has nothing to screen — replay is stronger"
        );
    }

    // ── Plausibility screening ──────────────────────────────────────────────

    #[test]
    fn plausible_event_is_admitted() {
        let g = PlausibilityGate::default();
        g.admit(&ev(pk(1), "swing", 1, 10_000), 10_000).unwrap();
    }

    #[test]
    fn superhuman_speed_is_refused() {
        let g = PlausibilityGate::default();
        let mut e = ev(pk(1), "swing", 1, 10_000);
        e.speed_mps = Some(400.0);
        assert!(matches!(
            g.admit(&e, 10_000),
            Err(Implausible::SpeedUnreachable { .. })
        ));
    }

    #[test]
    fn same_kind_inside_its_cooldown_is_refused() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            cooldown_ms: 500,
            ..Default::default()
        });
        g.admit(&ev(pk(1), "swing", 1, 10_000), 10_000).unwrap();
        assert!(matches!(
            g.admit(&ev(pk(1), "swing", 2, 10_100), 10_100),
            Err(Implausible::Cooldown { .. })
        ));
        // A different kind is on its own cooldown clock.
        g.admit(&ev(pk(1), "punch", 3, 10_100), 10_100).unwrap();
        // …and the same kind after the cooldown is fine again.
        g.admit(&ev(pk(1), "swing", 4, 10_600), 10_600).unwrap();
    }

    #[test]
    fn rate_limit_caps_events_per_second_per_player() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            max_events_per_sec: 3,
            cooldown_ms: 0,
            ..Default::default()
        });
        for seq in 1..=3 {
            g.admit(&ev(pk(1), "swing", seq, 10_000 + seq), 10_000 + seq)
                .unwrap();
        }
        assert!(matches!(
            g.admit(&ev(pk(1), "swing", 9, 10_010), 10_010),
            Err(Implausible::RateExceeded { .. })
        ));
        // Another player has their own budget.
        g.admit(&ev(pk(2), "swing", 1, 10_010), 10_010).unwrap();
        // The window slides.
        g.admit(&ev(pk(1), "swing", 10, 11_500), 11_500).unwrap();
    }

    #[test]
    fn rejected_events_do_not_consume_the_rate_budget() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            max_events_per_sec: 2,
            cooldown_ms: 0,
            ..Default::default()
        });
        // Ten refusals…
        let mut bad = ev(pk(1), "swing", 1, 10_000);
        bad.speed_mps = Some(999.0);
        for _ in 0..10 {
            assert!(g.admit(&bad, 10_000).is_err());
        }
        // …must not have spent the honest player's budget.
        g.admit(&ev(pk(1), "swing", 1, 10_000), 10_000).unwrap();
        g.admit(&ev(pk(1), "swing", 2, 10_001), 10_001).unwrap();
    }

    #[test]
    fn replayed_sequence_numbers_are_refused() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            cooldown_ms: 0,
            ..Default::default()
        });
        g.admit(&ev(pk(1), "swing", 5, 10_000), 10_000).unwrap();
        assert!(matches!(
            g.admit(&ev(pk(1), "swing", 5, 10_100), 10_100),
            Err(Implausible::SequenceReplayed)
        ));
        assert!(matches!(
            g.admit(&ev(pk(1), "swing", 4, 10_100), 10_100),
            Err(Implausible::SequenceReplayed)
        ));
        g.admit(&ev(pk(1), "swing", 6, 10_100), 10_100).unwrap();
    }

    #[test]
    fn nonsense_confidence_and_timestamps_are_refused() {
        let g = PlausibilityGate::default();
        let mut low = ev(pk(1), "swing", 1, 10_000);
        low.confidence = 0.01;
        assert_eq!(g.admit(&low, 10_000), Err(Implausible::ImplausibleConfidence));

        let mut nan = ev(pk(1), "swing", 1, 10_000);
        nan.confidence = f32::NAN;
        assert_eq!(g.admit(&nan, 10_000), Err(Implausible::ImplausibleConfidence));

        let mut over = ev(pk(1), "swing", 1, 10_000);
        over.confidence = 1.5;
        assert_eq!(g.admit(&over, 10_000), Err(Implausible::ImplausibleConfidence));

        assert_eq!(
            g.admit(&ev(pk(1), "swing", 1, 99_000_000), 10_000),
            Err(Implausible::FutureTimestamp)
        );
        assert_eq!(
            g.admit(&ev(pk(1), "swing", 1, 1_000), 10_000_000),
            Err(Implausible::StaleTimestamp)
        );
    }

    #[test]
    fn unlisted_kinds_are_refused_when_an_allowlist_is_set() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            accepted_kinds: vec!["swing".into()],
            ..Default::default()
        });
        g.admit(&ev(pk(1), "swing", 1, 10_000), 10_000).unwrap();
        assert_eq!(
            g.admit(&ev(pk(1), "teleport", 2, 10_500), 10_500),
            Err(Implausible::UnknownKind)
        );
    }

    // ── Signatures prove authorship only ────────────────────────────────────

    #[test]
    fn signed_event_roundtrips_and_verifies() {
        let player = RawKeypairAuth::from_seed([3u8; 32]);
        let e = ev(player.pubkey(), "swing", 1, 10_000);
        let s = SignedAttestedEvent::sign(&player, e);
        s.verify::<RawKeypairAuth>().unwrap();
        let back: SignedAttestedEvent =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        back.verify::<RawKeypairAuth>().unwrap();
    }

    #[test]
    fn nobody_can_sign_an_event_in_another_players_name() {
        let honest = RawKeypairAuth::from_seed([3u8; 32]);
        let attacker = RawKeypairAuth::from_seed([4u8; 32]);
        // Attacker signs an event claiming to be the honest player.
        let e = ev(honest.pubkey(), "swing", 1, 10_000);
        let s = SignedAttestedEvent::sign(&attacker, e);
        assert!(
            s.verify::<RawKeypairAuth>().is_err(),
            "the signing key must match the player named in the event"
        );
    }

    #[test]
    fn tampering_with_a_signed_event_body_fails_verification() {
        let player = RawKeypairAuth::from_seed([3u8; 32]);
        let mut s = SignedAttestedEvent::sign(&player, ev(player.pubkey(), "swing", 1, 10_000));
        s.event.speed_mps = Some(50.0);
        assert!(matches!(
            s.verify::<RawKeypairAuth>(),
            Err(SeamError::InvalidSignature)
        ));
    }

    /// The honesty test. A cheater's *own* synthetic events are correctly signed
    /// and, if they stay inside human bounds, pass every check this seam has.
    /// This is a property of sensor input, not a defect — the test exists so the
    /// limit is written down in code and cannot be quietly forgotten.
    #[test]
    fn a_plausible_synthetic_event_is_indistinguishable_from_a_real_one() {
        let cheater = RawKeypairAuth::from_seed([5u8; 32]);
        // Never touched a camera; hand-written numbers, well inside human range.
        let fabricated = AttestedEvent {
            player: cheater.pubkey(),
            kind: "swing".into(),
            confidence: 0.97,
            vector: Some([0.0, 1.0, 0.0]),
            speed_mps: Some(7.5),
            t_capture_ms: 10_000,
            seq: 1,
        };
        let signed = SignedAttestedEvent::sign(&cheater, fabricated.clone());
        signed.verify::<RawKeypairAuth>().unwrap();
        PlausibilityGate::default()
            .admit(&fabricated, 10_000)
            .expect("plausible synthetic input is accepted — there is no check that catches it");
    }

    #[tokio::test]
    async fn attested_ingress_counts_what_it_drops() {
        let p = AttestedEventInput::new(PlausibilityLimits {
            cooldown_ms: 0,
            max_age_ms: u64::MAX,
            max_future_skew_ms: u64::MAX,
            ..Default::default()
        });
        let player = RawKeypairAuth::from_seed([6u8; 32]);
        let good = SignedAttestedEvent::sign(&player, ev(player.pubkey(), "swing", 1, 10_000));
        p.submit_signed::<RawKeypairAuth>(&good).await.unwrap();

        let mut fast = ev(player.pubkey(), "swing", 2, 10_500);
        fast.speed_mps = Some(900.0);
        let bad = SignedAttestedEvent::sign(&player, fast);
        assert!(matches!(
            p.submit_signed::<RawKeypairAuth>(&bad).await,
            Err(SeamError::Implausible(_))
        ));

        assert_eq!(p.rejected(), 1);
        let drained = p.drain(0).await;
        assert_eq!(drained.len(), 1, "only the admitted event reached the sim");
        assert!(!drained[0].class().is_replay_verifiable());
    }

    #[tokio::test]
    async fn a_forged_signature_never_reaches_the_gate() {
        let p = AttestedEventInput::default();
        let honest = RawKeypairAuth::from_seed([3u8; 32]);
        let attacker = RawKeypairAuth::from_seed([4u8; 32]);
        let forged =
            SignedAttestedEvent::sign(&attacker, ev(honest.pubkey(), "swing", 1, 10_000));
        assert!(p.submit_signed::<RawKeypairAuth>(&forged).await.is_err());
        assert!(p.drain(0).await.is_empty());
    }

    #[test]
    fn forgetting_a_player_clears_only_their_state() {
        let g = PlausibilityGate::new(PlausibilityLimits {
            cooldown_ms: 0,
            ..Default::default()
        });
        g.admit(&ev(pk(1), "swing", 9, 10_000), 10_000).unwrap();
        g.admit(&ev(pk(2), "swing", 9, 10_000), 10_000).unwrap();
        g.forget(&pk(1));
        // Player 1 rejoined: their sequence counter starts over.
        g.admit(&ev(pk(1), "swing", 1, 10_100), 10_100).unwrap();
        // Player 2's is untouched.
        assert!(matches!(
            g.admit(&ev(pk(2), "swing", 1, 10_100), 10_100),
            Err(Implausible::SequenceReplayed)
        ));
    }
}
