//! Replay of the family's shared canonical-CBOR conformance corpus against
//! `magnetite_seams::cbor` — the third, hand-written codec implementation in
//! the family, and the one this corpus had never been checked against (see
//! `ALIGNMENT.md` §9 item 1, `docs/cross-repo-backlog.md` C2, and the
//! "TEMPORARY STATE" notice on `src/cbor.rs`).
//!
//! `magnetite_seams::cbor::Cv` is a strict subset of the value space the
//! corpus was generated from (`kotva_cbor::Value`, itself extracted from
//! evermesh): no negative integers, no `null`, and map keys are `u64` only —
//! "kotva-minus-TextMap", as the module docs put it. So most `accept` vectors
//! do not apply here at all: `Cv` cannot represent the value in question,
//! which is a structural gap in what the type can hold, not a codec
//! disagreement. This test does not hand-maintain a list of which vectors
//! that is — it classifies every vector by what
//! `magnetite_seams::cbor::decode` itself says, and asserts the resulting
//! counts so a corpus edit (or a decoder change) that moves them fails loudly
//! until the expected numbers are updated in the same commit.
//!
//! Does NOT depend on kotva or evermesh — products stay standalone. The
//! corpus is a frozen, committed copy
//! (`tests/vectors/evermesh-canonical-cbor.txt`) of kotva's copy of
//! evermesh's vectors; full provenance (both hops) is recorded in the file's
//! own header, not asserted here.

use magnetite_seams::cbor::{decode, encode, CborError, Cv};

const CORPUS: &str = include_str!("vectors/evermesh-canonical-cbor.txt");

/// Whole-corpus guard, independent of magnetite's narrower value space: proves
/// the file itself hasn't gone missing, been truncated, or silently drifted
/// from the copy kotva mechanically verified byte-for-byte against evermesh
/// (183/183 lines identical, 2026-07-30).
const EXPECT_TOTAL_ACCEPT: usize = 153;
const EXPECT_TOTAL_REJECT: usize = 30;

/// Of the 153 `accept` vectors, exactly this many decode under
/// `magnetite_seams::cbor` — i.e. use only `u64` map keys and never a negative
/// integer or `null` anywhere in the value tree. The other 144 exercise
/// evermesh's text-keyed per-kind record bodies (or, for one vector,
/// `json/negative-integer-key`'s `{-3: null}`), which `Cv` cannot represent at
/// all regardless of whether the bytes are canonical.
const EXPECT_SHARED_ACCEPT: usize = 9;
const EXPECT_STRUCTURALLY_EXCLUDED: usize = 144;

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex digit"))
        .collect()
}

struct Vector {
    accept: bool,
    label: String,
    bytes: Vec<u8>,
}

fn corpus() -> Vec<Vector> {
    let mut out = Vec::new();
    for (lineno, line) in CORPUS.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split_whitespace();
        let verdict = f.next().unwrap_or_default();
        let label = f.next().unwrap_or_default().to_string();
        let hex = f.next().unwrap_or_default();
        assert!(
            f.next().is_none(),
            "line {}: expected exactly 3 fields",
            lineno + 1
        );
        assert!(
            !label.is_empty() && !hex.is_empty(),
            "line {}: malformed vector",
            lineno + 1
        );
        let accept = match verdict {
            "accept" => true,
            "reject" => false,
            other => panic!("line {}: unknown verdict {other:?}", lineno + 1),
        };
        out.push(Vector {
            accept,
            label,
            bytes: unhex(hex),
        });
    }
    out
}

#[test]
fn corpus_is_present_and_the_expected_size() {
    let vs = corpus();
    let accept = vs.iter().filter(|v| v.accept).count();
    let reject = vs.len() - accept;
    assert_eq!(
        (accept, reject),
        (EXPECT_TOTAL_ACCEPT, EXPECT_TOTAL_REJECT),
        "corpus size moved: {accept} accept / {reject} reject. This file is a \
         frozen copy of kotva's copy of evermesh's vectors — if it changed on \
         purpose, update the expected counts (here, and in kotva) in the same \
         commit. A corpus that silently shrank would otherwise let this test \
         report success having checked nothing."
    );
}

/// A decode failure that is exactly the shape `Cv`'s narrower domain implies:
/// a negative integer, a `null`, or a non-`u64` map key anywhere in the value.
/// Anything else is not a structural gap — it is a real disagreement with the
/// corpus and must surface as a failure, never be folded into this bucket.
fn is_structural_gap(e: &CborError) -> bool {
    matches!(
        e,
        CborError::NegativeInteger | CborError::NullPresent | CborError::NonIntegerKey
    )
}

/// Every `accept` vector either applies to `Cv` (and must round-trip exactly —
/// the malleability guarantee this whole codec exists to provide) or is
/// excluded for one of exactly three named structural reasons — never because
/// it merely failed to decode for some other cause, and never silently.
///
/// **If this test ever fails on a `divergences` assertion, DO NOT weaken it,
/// exclude the vector, or change the expected counts to match. That failure
/// IS the finding this test exists to produce: a real byte-level disagreement
/// between `magnetite_seams::cbor` and the rest of the family.**
#[test]
fn accept_vectors_split_into_shared_subset_and_structural_exclusions() {
    let mut in_subset = 0usize;
    let mut excluded = 0usize;
    let mut divergences: Vec<String> = Vec::new();

    for v in corpus().iter().filter(|v| v.accept) {
        match decode(&v.bytes) {
            Ok(value) => {
                in_subset += 1;
                let again = encode(&value);
                if again != v.bytes {
                    divergences.push(format!(
                        "{}: decoded then RE-ENCODED TO DIFFERENT BYTES (the \
                         malleability guarantee is broken).\n  expected: {}\n  \
                         actual:   {}",
                        v.label,
                        hex(&v.bytes),
                        hex(&again),
                    ));
                }
            }
            Err(e) if is_structural_gap(&e) => {
                excluded += 1;
            }
            Err(e) => {
                divergences.push(format!(
                    "{}: kotva/evermesh classify this as canonical (accept), but \
                     magnetite_seams::cbor::decode refused it for a reason OTHER \
                     than its documented type gap (NegativeInteger / NullPresent \
                     / NonIntegerKey): got {e:?}\n  bytes: {}",
                    v.label,
                    hex(&v.bytes),
                ));
            }
        }
    }

    assert!(
        divergences.is_empty(),
        "\n\n=== magnetite_seams::cbor DIVERGES from the shared corpus ===\n\n{}\n",
        divergences.join("\n\n")
    );
    assert_eq!(
        in_subset, EXPECT_SHARED_ACCEPT,
        "shared-subset accept count moved (vectors magnetite's Cv can represent)"
    );
    assert_eq!(
        excluded, EXPECT_STRUCTURALLY_EXCLUDED,
        "structural-exclusion count moved (vectors Cv cannot represent at all)"
    );
    assert_eq!(
        in_subset + excluded,
        EXPECT_TOTAL_ACCEPT,
        "every accept vector must be accounted for as either in-subset or excluded"
    );
}

/// Every `reject` vector applies unconditionally, with no exclusion possible:
/// refusal is not type-specific, so `Cv`'s narrower domain can only refuse a
/// superset of what `kotva_cbor::decode_canonical` refuses. If
/// `magnetite_seams::cbor::decode` ever ACCEPTS bytes this corpus marks
/// `reject`, that is a genuine over-permissive divergence — the decoder would
/// be handing a verifier a second, non-canonical encoding of a signed object.
#[test]
fn every_reject_vector_is_refused() {
    let mut checked = 0usize;
    let mut accepted_when_it_should_not_be: Vec<String> = Vec::new();

    for v in corpus().iter().filter(|v| !v.accept) {
        match decode(&v.bytes) {
            Ok(_) => accepted_when_it_should_not_be.push(format!(
                "{}: ACCEPTED bytes the corpus marks reject (non-canonical or \
                 otherwise invalid)\n  bytes: {}",
                v.label,
                hex(&v.bytes),
            )),
            Err(_) => checked += 1,
        }
    }

    assert!(
        accepted_when_it_should_not_be.is_empty(),
        "\n\n=== magnetite_seams::cbor is OVER-PERMISSIVE vs. the shared corpus ===\n\n{}\n",
        accepted_when_it_should_not_be.join("\n\n")
    );
    assert_eq!(
        checked, EXPECT_TOTAL_REJECT,
        "not every reject vector was checked"
    );
}

/// The shared subset is small — evermesh's per-kind record bodies are
/// text-keyed and `Cv` cannot hold a text-keyed map at all — but it must not
/// be degenerate. Prove it actually exercises real shapes (byte strings,
/// arrays), not 9 copies of the same all-zero scalar.
#[test]
fn shared_subset_is_not_degenerate() {
    fn walk(c: &Cv, byte_strings: &mut usize, arrays: &mut usize, maps: &mut usize) {
        match c {
            Cv::Bytes(_) => *byte_strings += 1,
            Cv::Array(items) => {
                *arrays += 1;
                for i in items {
                    walk(i, byte_strings, arrays, maps);
                }
            }
            Cv::Map(entries) => {
                *maps += 1;
                for (_, v) in entries {
                    walk(v, byte_strings, arrays, maps);
                }
            }
            _ => {}
        }
    }

    let mut byte_strings = 0usize;
    let mut arrays = 0usize;
    let mut maps = 0usize;
    let mut distinct_labels = std::collections::BTreeSet::new();

    for v in corpus().iter().filter(|v| v.accept) {
        if let Ok(value) = decode(&v.bytes) {
            distinct_labels.insert(v.label.clone());
            walk(&value, &mut byte_strings, &mut arrays, &mut maps);
        }
    }

    assert_eq!(distinct_labels.len(), EXPECT_SHARED_ACCEPT);
    assert!(
        byte_strings > 0,
        "shared subset never exercises a byte string"
    );
    assert!(arrays > 0, "shared subset never exercises an array");
    assert!(maps > 0, "shared subset never exercises a nested map");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
