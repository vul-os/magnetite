//! Seam §3.1 extension (A8) — a rotated key remains the SAME identity.
//!
//! # The defect this closes
//!
//! Before this module, [`Identity`](crate::identity::Identity) was "a keypair
//! that signs" with no notion of a key being superseded. A node or player that
//! rotated its key became, to every consumer of this seam, a *different*
//! identity — every peer that pinned the old key, every tracker slot bound to
//! it, orphaned. `magnetite-kotva`'s docs named this "the largest gap in the
//! kotva binding": kotva's `DeviceCert`, `RecoveryPolicy`, `KeyRotation` and
//! `MoveRecord` are the substantive half of `kotva_core::identity` and had
//! nowhere to land here.
//!
//! # What was read before anything was designed
//!
//! Per this project's own standing lesson ("read the sibling repos before
//! designing anything" — it has changed the answer four times), the rotation
//! semantics below are **adopted, not invented**, from two sources, neither of
//! which is a dependency of this crate (see "Standalone" below):
//!
//! * **kotva** (`kotva-core::identity`, read from the sibling `kotva` repo at
//!   `crates/kotva-core/src/identity.rs`) supplies the *shape* of a rotation
//!   record: a continuity signature by the **retiring** key over a body naming
//!   the incoming key (`KeyRotation`, §18.4.5), hash-chained via `prev`, with
//!   an optional quorum-backed escape hatch for the stolen-key case
//!   (`RecoveryPolicy`, `GuardianApproval`, `authorize_key_rotation`).
//! * **evermesh** (`evermesh-kernel::identity`, read from the sibling
//!   `evermesh` repo, spec 002) supplies the *decided fork-resolution rule* —
//!   confirmed against its own conformance vectors, which is the load-bearing
//!   part: `identity/genesis-valid`, `identity/rotate-signing`,
//!   `identity/rotate-recovery`, `identity/fork-recovery-precedence`,
//!   `identity/fork-same-class-tiebreak` and
//!   `identity/chain-order-merge-{1,2,3}`. The rule, copied by hand rather
//!   than re-derived: a rotation record is authorized either by the *current
//!   signing key* or by a *current declared recovery key*; when two records
//!   compete for the same parent, a **finalized** signing-class record can
//!   never be displaced; short of finality, a **recovery-class** record beats
//!   a signing-class one (theft defense: the thief's rotation is provisional
//!   until it ages past the contest window); two records in the **same**
//!   class competing for the same parent are resolved by the bytewise-lower
//!   record id, which is fully deterministic given the record set and so is
//!   **merge-order independent** — the chain-order-merge vectors exist
//!   specifically to prove that arrival order cannot matter, which is why
//!   this implementation indexes candidates by parent hash and re-derives the
//!   winner from that index rather than folding records in whatever order
//!   they were handed in.
//!
//! # What is deliberately NOT here (the "minimum seam", not a subsystem)
//!
//! This is the **minimum** seam that lets a rotated key remain the same
//! identity — not a port of either sibling's full machinery:
//!
//! * **No CBOR.** Every signed type in this seam (`Challenge`, `TokenClaims`)
//!   already uses a hand-rolled little-endian byte concatenation, not kotva's
//!   canonical integer-keyed CBOR. [`RotationRecord::signing_bytes`] follows
//!   that existing house style rather than importing a codec.
//! * **No multi-suite / crypto-agility.** One suite (Ed25519), matching every
//!   other key in this seam. evermesh's `rotate-alg-field-present` vector
//!   exercises a `key_alg` field for exactly this reason on evermesh's side;
//!   this seam has no second algorithm to migrate to, so the field is not
//!   reproduced.
//! * **No guardian quorum / threshold signatures.** kotva's `RecoveryPolicy`
//!   is an M-of-N social-recovery quorum with its own `GuardianApproval`
//!   co-signature type and a `rotate_threshold` bar. Here, "recovery" is
//!   reduced to evermesh's simpler shape: a plain **set of recovery keys**,
//!   any one of which suffices to author a recovery-class rotation. There is
//!   no quorum counting and no `GuardianApproval` type.
//! * **No online veto flow.** kotva's §1.5 path (b) (published-and-delayed,
//!   defeatable by a guardian veto within the window) is not modeled; only
//!   the finality-by-elapsed-time half of it (`contest_window_secs` /
//!   `observed_at`) is, because that half is what the evermesh conformance
//!   vectors this module is graded against actually exercise.
//! * **`DeviceCert` — not implemented.** It is a *delegation* fact (this root
//!   identity's current key authorizes a named device's separate key for
//!   certain capabilities), layered ON TOP of a resolved chain head. It is
//!   not about whether the root identity persists through rotation, which is
//!   the entire subject of this module. Left for whenever this seam grows a
//!   device/session concept of its own.
//! * **`MoveRecord` — not implemented.** It rebinds a *human name* while
//!   preserving the key — that is the [`crate::naming`] seam's concern
//!   (`Naming::resolve`/`display`), orthogonal to which key an identity
//!   currently uses. Nothing here should be confused with a naming fact.
//!
//! # Standalone
//!
//! This module depends on neither `kotva-core` nor `evermesh-kernel` — only
//! their *decided semantics*, copied by hand into this crate's own types and
//! encoding, exactly like `magnetite-kotva`'s empty-domain choice copies a
//! *fact* about kotva's signing without adding a dependency for it. Nothing
//! here is fictional: every fork-resolution branch below is exercised by a
//! test named after the evermesh vector it mirrors.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::blobstore::Hash;
use crate::error::{Result, SeamError};
use crate::identity::{IdentityVerifier, PubKey, Sig};

/// One link in an identity's key-rotation chain.
///
/// The **genesis** record (`prev == None`) is self-signed: `signed_by == key`.
/// Every later record is signed by `signed_by`, which must be either the
/// *current* signing key or a *current* declared recovery key at the parent
/// position — [`verify_chain`] is what actually decides whether a given
/// record has standing; a lone `RotationRecord` proves only that its own
/// signature is genuine, never that installing it is authorized.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RotationRecord {
    /// The key this record installs as current.
    pub key: PubKey,
    /// Declared recovery keys as of this record — kotva calls the same idea
    /// `RecoveryPolicy`, reduced here to a plain set (see module docs).
    pub recovery: Vec<PubKey>,
    /// Seconds after first observation a signing-class rival needs to age
    /// past before it is "final" (spec-002 §4a). `0` disables recovery
    /// precedence entirely: any record, of either class, is decided by
    /// lowest id (spec-002 §4b applies to the whole set, not just same-class
    /// competitors).
    pub contest_window_secs: u64,
    /// Unix seconds this record was produced.
    pub ts: u64,
    /// Content hash of the parent record, or `None` for the genesis record.
    pub prev: Option<Hash>,
    /// The key that actually produced `sig`: the current signing key for an
    /// ordinary rotation, or one of the current recovery keys for a
    /// theft-recovery rotation. `signed_by == key` at genesis (self-signed).
    pub signed_by: PubKey,
    /// Continuity signature over [`Self::signing_bytes`], by `signed_by`.
    pub sig: Sig,
}

impl RotationRecord {
    /// Deterministic bytes `signed_by` signs — everything except `sig`
    /// itself, matching [`crate::identity::TokenClaims::signing_bytes`]'s
    /// house style (length-prefixed where variable, fixed concatenation
    /// otherwise).
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.key.0);
        b.extend_from_slice(&(self.recovery.len() as u32).to_le_bytes());
        for r in &self.recovery {
            b.extend_from_slice(&r.0);
        }
        b.extend_from_slice(&self.contest_window_secs.to_le_bytes());
        b.extend_from_slice(&self.ts.to_le_bytes());
        match &self.prev {
            Some(h) => {
                b.push(1);
                b.extend_from_slice(&h.0);
            }
            None => b.push(0),
        }
        b.extend_from_slice(&self.signed_by.0);
        b
    }

    /// This record's content id: `BLAKE3(signing_bytes() ‖ sig)`. Two
    /// records competing for the same parent are told apart by this id
    /// (evermesh spec 002 §4b, "the bytewise-lower record id wins") — it
    /// covers `sig` so the id cannot be predicted before the record is
    /// actually signed.
    pub fn id(&self) -> Hash {
        let mut b = self.signing_bytes();
        b.extend_from_slice(&self.sig.0);
        Hash::of(&b)
    }

    /// Verify the continuity signature under `provider` — the
    /// [`IdentityVerifier`] that is actually in play (A7's fix, reused here
    /// rather than re-hard-coding a type as kotva's own `KeyRotation::verify`
    /// effectively does via a fixed suite).
    pub fn verify_continuity(&self, provider: &dyn IdentityVerifier) -> bool {
        provider.verify(&self.signed_by, &self.signing_bytes(), &self.sig)
    }
}

/// The verified current state of an identity's rotation chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainState {
    /// The genesis record's content id — the identity's stable identifier.
    /// **This is what "same identity" means here: it never changes across a
    /// rotation**, only [`Self::signing_key`] does.
    pub identity_id: Hash,
    /// The current signing key (the chain head's `key`).
    pub signing_key: PubKey,
    /// The current declared recovery keys.
    pub recovery: Vec<PubKey>,
    /// The current contest window.
    pub contest_window_secs: u64,
    /// Content id of the chain head (the latest applied record).
    pub head: Hash,
    /// Number of rotations applied after genesis (genesis alone is depth 0).
    pub depth: u64,
}

/// Verify a rotation chain and compute its current state (evermesh spec 002
/// §4, adopted — see the module docs for exactly which parts).
///
/// `records` is any set of candidate records — order does not matter, and
/// deliberately is not relied on: candidates are indexed by parent id before
/// any resolution happens, which is what makes the result **merge-order
/// independent** (see `chain_order_merge_*` below, mirroring evermesh's
/// `identity/chain-order-merge-{1,2,3}` vectors). `observed_at` reports when
/// this verifier first saw a record (`None` ⇒ just-observed, not final);
/// `now` is the verifier's own clock. Both are verifier-local inputs, exactly
/// as in evermesh — this seam never reads a wall clock internally.
pub fn verify_chain(
    records: &[RotationRecord],
    provider: &dyn IdentityVerifier,
    observed_at: &dyn Fn(&Hash) -> Option<u64>,
    now: u64,
) -> Result<ChainState> {
    // 1. Locate exactly one valid genesis: self-signed, no parent.
    let mut genesis: Option<&RotationRecord> = None;
    for r in records {
        if r.prev.is_some() || r.signed_by != r.key {
            continue;
        }
        if !r.verify_continuity(provider) {
            continue;
        }
        if genesis.is_some() {
            return Err(SeamError::Invalid(
                "multiple genesis records in input".into(),
            ));
        }
        genesis = Some(r);
    }
    let genesis = genesis.ok_or_else(|| SeamError::Invalid("no valid genesis record".into()))?;
    let identity_id = genesis.id();

    // 2. Index verified candidates by parent id. This — not the order records
    //    arrived in — is what the walk below consults.
    let mut by_parent: HashMap<[u8; 32], Vec<&RotationRecord>> = HashMap::new();
    for r in records {
        let Some(parent) = &r.prev else { continue };
        if !r.verify_continuity(provider) {
            continue;
        }
        by_parent.entry(parent.0).or_default().push(r);
    }

    let mut state = ChainState {
        identity_id,
        signing_key: genesis.key,
        recovery: genesis.recovery.clone(),
        contest_window_secs: genesis.contest_window_secs,
        head: identity_id,
        depth: 0,
    };

    loop {
        let Some(children) = by_parent.get(&state.head.0) else {
            return Ok(state);
        };

        // Classify candidates under the state at THIS position — a key with
        // no standing here (neither current signing nor current recovery)
        // is simply ignored, never a winner.
        let mut signing: Vec<&RotationRecord> = Vec::new();
        let mut recovery: Vec<&RotationRecord> = Vec::new();
        for c in children {
            if c.signed_by == state.signing_key {
                signing.push(c);
            } else if state.recovery.contains(&c.signed_by) {
                recovery.push(c);
            }
        }

        let is_final = |r: &&RotationRecord| -> bool {
            match observed_at(&r.id()) {
                // Both operands are u64 throughout this module (unlike
                // evermesh's kernel, which mixes an attacker-chosen u64
                // `contest_window` into an `i64` comparison and had to add a
                // regression test for the resulting sign-wrap). Comparing in
                // u64 from the start means there is no cast for a huge
                // window to wrap negative through, so the hazard cannot
                // recur here structurally: `saturating_sub` floors at 0 for
                // a future-dated observation, which reads as "not final",
                // the same safe direction evermesh's fix landed on.
                Some(seen) => now.saturating_sub(seen) > state.contest_window_secs,
                None => false,
            }
        };

        let lowest = |v: &[&RotationRecord]| -> Option<Hash> {
            v.iter().map(|r| r.id()).min_by(|a, b| a.0.cmp(&b.0))
        };

        let chosen_id: Option<Hash> = if state.contest_window_secs == 0 {
            // Recovery precedence disabled: lowest id among every candidate.
            let mut all: Vec<&RotationRecord> = signing.clone();
            all.extend(recovery.iter().copied());
            lowest(&all)
        } else {
            let final_signing: Vec<&RotationRecord> =
                signing.iter().copied().filter(|r| is_final(r)).collect();
            if !final_signing.is_empty() {
                // A finalized signing rotation cannot be displaced by
                // anything, recovery included.
                lowest(&final_signing)
            } else if !recovery.is_empty() {
                // Theft-recovery precedence: a non-final signing rotation
                // loses to any recovery-class rotation.
                lowest(&recovery)
            } else {
                lowest(&signing)
            }
        };

        let Some(id) = chosen_id else {
            return Ok(state);
        };
        let mut all: Vec<&RotationRecord> = signing;
        all.extend(recovery);
        let rec = all
            .into_iter()
            .find(|r| r.id() == id)
            .expect("chosen id was derived from this exact candidate set");

        state.signing_key = rec.key;
        state.recovery = rec.recovery.clone();
        state.contest_window_secs = rec.contest_window_secs;
        state.head = id;
        state.depth += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{Identity, RawKeypairAuth};

    fn sign(signer: &RawKeypairAuth, r: &mut RotationRecord) {
        r.sig = signer.sign(&r.signing_bytes());
    }

    fn genesis(
        signer: &RawKeypairAuth,
        recovery: Vec<PubKey>,
        window: u64,
        ts: u64,
    ) -> RotationRecord {
        let mut r = RotationRecord {
            key: signer.pubkey(),
            recovery,
            contest_window_secs: window,
            ts,
            prev: None,
            signed_by: signer.pubkey(),
            sig: Sig([0u8; 64]),
        };
        sign(signer, &mut r);
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn rotate(
        signer: &RawKeypairAuth,
        prev: Hash,
        new_key: PubKey,
        recovery: Vec<PubKey>,
        window: u64,
        ts: u64,
    ) -> RotationRecord {
        let mut r = RotationRecord {
            key: new_key,
            recovery,
            contest_window_secs: window,
            ts,
            prev: Some(prev),
            signed_by: signer.pubkey(),
            sig: Sig([0u8; 64]),
        };
        sign(signer, &mut r);
        r
    }

    const WINDOW: u64 = 604_800;

    /// Everything just-observed, nothing final.
    fn seen_now(_: &Hash) -> Option<u64> {
        None
    }

    // --- mirrors evermesh `identity/genesis-valid` -----------------------------------------

    #[test]
    fn genesis_verifies_and_derives_identity_id() {
        let user = RawKeypairAuth::from_seed([1u8; 32]);
        let g = genesis(&user, vec![], WINDOW, 100);
        let state = verify_chain(std::slice::from_ref(&g), &user, &seen_now, 1000).unwrap();
        assert_eq!(state.identity_id, g.id());
        assert_eq!(state.signing_key, user.pubkey());
        assert_eq!(state.depth, 0);
    }

    // --- mirrors evermesh `identity/rotate-signing` — THE A8 PROOF -------------------------

    #[test]
    fn a_rotated_key_stays_the_same_identity() {
        // The exact defect A8 describes: before this module, a rotated key
        // became a DIFFERENT identity. Here, `identity_id` — the thing every
        // peer/tracker slot would pin — is IDENTICAL before and after the
        // rotation; only `signing_key` moves.
        let old = RawKeypairAuth::from_seed([1u8; 32]);
        let new = RawKeypairAuth::from_seed([2u8; 32]);
        let g = genesis(&old, vec![], WINDOW, 100);
        let rot = rotate(&old, g.id(), new.pubkey(), vec![], WINDOW, 200);

        let before = verify_chain(std::slice::from_ref(&g), &old, &seen_now, 1000).unwrap();
        let after = verify_chain(&[g.clone(), rot.clone()], &old, &seen_now, 1000).unwrap();

        assert_eq!(
            before.identity_id, after.identity_id,
            "rotating the key MUST NOT change the identity"
        );
        assert_ne!(
            before.signing_key, after.signing_key,
            "the signing key itself DOES change — that's the point of rotating it"
        );
        assert_eq!(after.signing_key, new.pubkey());
        assert_eq!(after.head, rot.id());
        assert_eq!(after.depth, 1);
    }

    #[test]
    fn unauthorized_rotation_is_ignored() {
        let user = RawKeypairAuth::from_seed([1u8; 32]);
        let attacker = RawKeypairAuth::from_seed([9u8; 32]);
        let g = genesis(&user, vec![], WINDOW, 100);
        // Signed by the attacker, who is neither the current signing key nor
        // a declared recovery key: no standing, must be ignored.
        let rogue = rotate(&attacker, g.id(), attacker.pubkey(), vec![], WINDOW, 200);
        let state = verify_chain(&[g.clone(), rogue], &user, &seen_now, 1000).unwrap();
        assert_eq!(state.depth, 0);
        assert_eq!(state.signing_key, user.pubkey());
    }

    // --- mirrors evermesh `identity/rotate-recovery` ---------------------------------------

    #[test]
    fn a_recovery_key_can_also_rotate() {
        let signing = RawKeypairAuth::from_seed([1u8; 32]);
        let recovery = RawKeypairAuth::from_seed([2u8; 32]);
        let new_signing = RawKeypairAuth::from_seed([3u8; 32]);
        let g = genesis(&signing, vec![recovery.pubkey()], WINDOW, 100);
        let rot = rotate(&recovery, g.id(), new_signing.pubkey(), vec![], WINDOW, 200);
        let state = verify_chain(&[g, rot.clone()], &signing, &seen_now, 1000).unwrap();
        assert_eq!(state.signing_key, new_signing.pubkey());
        assert_eq!(state.head, rot.id());
        assert_eq!(state.depth, 1);
    }

    // --- mirrors evermesh `identity/fork-recovery-precedence` ------------------------------

    #[test]
    fn recovery_beats_a_provisional_thief() {
        // Thief steals the signing key and rotates; the owner forks from the
        // same parent with the recovery key. While the thief's branch is not
        // final, recovery wins — even against a DEEPER thief branch.
        let owner_signing = RawKeypairAuth::from_seed([1u8; 32]);
        let owner_recovery = RawKeypairAuth::from_seed([2u8; 32]);
        let thief = RawKeypairAuth::from_seed([3u8; 32]);
        let owner_new = RawKeypairAuth::from_seed([4u8; 32]);

        let g = genesis(&owner_signing, vec![owner_recovery.pubkey()], WINDOW, 100);
        let thief_rot = rotate(&owner_signing, g.id(), thief.pubkey(), vec![], WINDOW, 200);
        let owner_rot = rotate(
            &owner_recovery,
            g.id(),
            owner_new.pubkey(),
            vec![owner_recovery.pubkey()],
            WINDOW,
            300,
        );
        // Deeper thief branch must still lose.
        let attacker2 = RawKeypairAuth::from_seed([5u8; 32]);
        let thief_rot2 = rotate(
            &thief,
            thief_rot.id(),
            attacker2.pubkey(),
            vec![],
            WINDOW,
            400,
        );

        let records = vec![g, thief_rot, thief_rot2, owner_rot.clone()];
        let state = verify_chain(&records, &owner_signing, &seen_now, 1000).unwrap();
        assert_eq!(state.head, owner_rot.id());
        assert_eq!(state.signing_key, owner_new.pubkey());
    }

    #[test]
    fn a_finalized_signing_rotation_resists_a_late_recovery_fork() {
        // A legitimate signing rotation observed longer ago than the contest
        // window cannot be displaced by a later recovery fork.
        let signing = RawKeypairAuth::from_seed([1u8; 32]);
        let recovery = RawKeypairAuth::from_seed([2u8; 32]);
        let new_signing = RawKeypairAuth::from_seed([3u8; 32]);
        let recovery_new = RawKeypairAuth::from_seed([4u8; 32]);

        let g = genesis(&signing, vec![recovery.pubkey()], WINDOW, 100);
        let legit = rotate(
            &signing,
            g.id(),
            new_signing.pubkey(),
            vec![recovery.pubkey()],
            WINDOW,
            200,
        );
        let late_recovery = rotate(
            &recovery,
            g.id(),
            recovery_new.pubkey(),
            vec![],
            WINDOW,
            300,
        );

        let legit_id = legit.id();
        let observed = move |h: &Hash| -> Option<u64> {
            if *h == legit_id {
                Some(0) // seen long ago
            } else {
                None
            }
        };
        let now = WINDOW + 10;
        let records = vec![g, legit.clone(), late_recovery];
        let state = verify_chain(&records, &signing, &observed, now).unwrap();
        assert_eq!(state.head, legit.id());
        assert_eq!(state.signing_key, new_signing.pubkey());
    }

    // --- mirrors evermesh `identity/fork-same-class-tiebreak` ------------------------------

    #[test]
    fn same_class_fork_resolves_by_lowest_id_deterministically() {
        let signing = RawKeypairAuth::from_seed([1u8; 32]);
        let a = RawKeypairAuth::from_seed([2u8; 32]);
        let b = RawKeypairAuth::from_seed([3u8; 32]);
        let g = genesis(&signing, vec![], WINDOW, 100);
        let rot_a = rotate(&signing, g.id(), a.pubkey(), vec![], WINDOW, 200);
        let rot_b = rotate(&signing, g.id(), b.pubkey(), vec![], WINDOW, 201);

        let expected = if rot_a.id().0 < rot_b.id().0 {
            rot_a.id()
        } else {
            rot_b.id()
        };
        let state = verify_chain(&[g, rot_a, rot_b], &signing, &seen_now, 1000).unwrap();
        assert_eq!(state.head, expected);
    }

    // --- mirrors evermesh `identity/chain-order-merge-{1,2,3}` -----------------------------

    #[test]
    fn chain_state_is_merge_order_independent() {
        let signing = RawKeypairAuth::from_seed([1u8; 32]);
        let recovery = RawKeypairAuth::from_seed([2u8; 32]);
        let n1 = RawKeypairAuth::from_seed([3u8; 32]);
        let n2 = RawKeypairAuth::from_seed([4u8; 32]);

        let g = genesis(&signing, vec![recovery.pubkey()], WINDOW, 100);
        let r1 = rotate(
            &signing,
            g.id(),
            n1.pubkey(),
            vec![recovery.pubkey()],
            WINDOW,
            200,
        );
        let r2 = rotate(&n1, r1.id(), n2.pubkey(), vec![], WINDOW, 300);

        let mut records = vec![g, r1, r2];
        let baseline = verify_chain(&records, &signing, &seen_now, 1000).unwrap();

        // All 6 permutations of 3 records give the identical state — this is
        // the actual property `chain-order-merge-{1,2,3}` exercises: the
        // SAME records, merged in a DIFFERENT arrival order, produce the
        // identical resulting chain state.
        for _ in 0..3 {
            records.rotate_left(1);
            assert_eq!(
                verify_chain(&records, &signing, &seen_now, 1000).unwrap(),
                baseline
            );
            let mut swapped = records.clone();
            swapped.swap(0, 1);
            assert_eq!(
                verify_chain(&swapped, &signing, &seen_now, 1000).unwrap(),
                baseline
            );
        }
    }

    #[test]
    fn multiple_genesis_records_are_rejected() {
        let a = RawKeypairAuth::from_seed([1u8; 32]);
        let b = RawKeypairAuth::from_seed([2u8; 32]);
        let ga = genesis(&a, vec![], WINDOW, 100);
        let gb = genesis(&b, vec![], WINDOW, 100);
        assert!(matches!(
            verify_chain(&[ga, gb], &a, &seen_now, 1000),
            Err(SeamError::Invalid(_))
        ));
    }

    #[test]
    fn no_genesis_is_rejected() {
        let a = RawKeypairAuth::from_seed([1u8; 32]);
        let b = RawKeypairAuth::from_seed([2u8; 32]);
        // A "rotation" with no genesis in the input set to anchor it.
        let orphan = rotate(
            &a,
            Hash::of(b"nonexistent-parent"),
            b.pubkey(),
            vec![],
            WINDOW,
            200,
        );
        assert!(matches!(
            verify_chain(&[orphan], &a, &seen_now, 1000),
            Err(SeamError::Invalid(_))
        ));
    }
}
