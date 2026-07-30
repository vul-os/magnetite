//! Replay of evermesh's chunk-tree conformance vectors (backlog item A24)
//! against `magnetite_seams::chunktree` — the independent, by-hand copy of
//! evermesh's `EM-1` chunk-tree profile (see `src/chunktree.rs`'s module
//! docs for exactly what was adopted and what was deliberately left out).
//!
//! The 14 vector files under `tests/vectors/evermesh-chunktree/` are a
//! byte-for-byte copy of evermesh's own
//! `tools/conformance/vectors/chunktree/*.json` (provenance, including a
//! sha256 per file, recorded in that directory's `PROVENANCE.txt` — never
//! hand-edit a vector file). Unlike the CBOR corpus replay
//! (`tests/evermesh_conformance.rs`), there is no narrower value domain here
//! to structurally exclude a vector against: `verify_chunk` operates on raw
//! bytes and fixed-size `[u8; 32]` hashes with no analogue of `Cv`'s
//! restricted map-key/negative-integer domain, so every vector that exists
//! is expected to replay, and the test asserts that count directly rather
//! than carving out an "excluded" bucket that would only be inviting one to
//! reappear silently.
//!
//! Does NOT depend on kotva or evermesh — products stay standalone
//! (`FANOUT-LOOP-STATE.md` owner directive #1). The vectors are a frozen,
//! committed copy; full provenance is in the vectors directory's own header,
//! not asserted here.

use magnetite_seams::chunktree::{verify_chunk, CHUNK_SIZE};

/// Every vector file, replayed via `include_str!` (a literal list, not a
/// directory walk) so the count that gets compiled in is visible at a
/// glance and cannot silently shrink if a file goes missing from disk
/// without also being removed from this list.
const VECTOR_FILES: &[(&str, &str)] = &[
    (
        "empty-blob-any-index-invalid",
        include_str!("vectors/evermesh-chunktree/empty-blob-any-index-invalid.json"),
    ),
    (
        "invalid-wrong-index",
        include_str!("vectors/evermesh-chunktree/invalid-wrong-index.json"),
    ),
    (
        "invalid-wrong-sibling",
        include_str!("vectors/evermesh-chunktree/invalid-wrong-sibling.json"),
    ),
    (
        "valid-1-chunks-index-0",
        include_str!("vectors/evermesh-chunktree/valid-1-chunks-index-0.json"),
    ),
    (
        "valid-2-chunks-index-0",
        include_str!("vectors/evermesh-chunktree/valid-2-chunks-index-0.json"),
    ),
    (
        "valid-2-chunks-index-1",
        include_str!("vectors/evermesh-chunktree/valid-2-chunks-index-1.json"),
    ),
    (
        "valid-3-chunks-index-0",
        include_str!("vectors/evermesh-chunktree/valid-3-chunks-index-0.json"),
    ),
    (
        "valid-3-chunks-index-1",
        include_str!("vectors/evermesh-chunktree/valid-3-chunks-index-1.json"),
    ),
    (
        "valid-3-chunks-index-2",
        include_str!("vectors/evermesh-chunktree/valid-3-chunks-index-2.json"),
    ),
    (
        "valid-5-chunks-index-0",
        include_str!("vectors/evermesh-chunktree/valid-5-chunks-index-0.json"),
    ),
    (
        "valid-5-chunks-index-1",
        include_str!("vectors/evermesh-chunktree/valid-5-chunks-index-1.json"),
    ),
    (
        "valid-5-chunks-index-2",
        include_str!("vectors/evermesh-chunktree/valid-5-chunks-index-2.json"),
    ),
    (
        "valid-5-chunks-index-3",
        include_str!("vectors/evermesh-chunktree/valid-5-chunks-index-3.json"),
    ),
    (
        "valid-5-chunks-index-4",
        include_str!("vectors/evermesh-chunktree/valid-5-chunks-index-4.json"),
    ),
];

/// Total vector count this file claims to replay. Asserted directly, not
/// inferred from `VECTOR_FILES.len()`, so a silent addition/removal in the
/// list above without updating this constant fails loudly rather than
/// quietly changing what "replay the 14 vectors" means.
const EXPECT_TOTAL: usize = 14;
const EXPECT_VALID: usize = 11;
const EXPECT_INVALID: usize = 3;

/// Vectors excluded from replay because `magnetite_seams::chunktree` cannot
/// structurally represent them. Zero: `verify_chunk` takes raw chunk bytes
/// and fixed-size `[u8; 32]` root/sibling hashes, which is exactly evermesh's
/// own `EM-1` shape — there is no narrower value domain (no analogue of
/// `magnetite_seams::cbor::Cv`'s restricted map-key/no-negative-integer
/// domain) that could make a vector inapplicable. Asserted explicitly so
/// that if a future vector genuinely cannot be replayed, this constant has
/// to change in the same commit as the reason for it, rather than the
/// vector being silently dropped from `VECTOR_FILES`.
const EXPECT_EXCLUDED: usize = 0;

fn unhex32(s: &str) -> [u8; 32] {
    let bytes = unhex(s);
    bytes.try_into().unwrap_or_else(|b: Vec<u8>| {
        panic!("expected a 32-byte hex string, got {} bytes: {s}", b.len())
    })
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "odd-length hex: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex digit"))
        .collect()
}

/// Reconstruct the synthetic blob a `chunk-proof` vector describes: chunk
/// `i` filled with `(i % 251)` for its whole length, every chunk but the
/// last exactly `CHUNK_SIZE`. Mirrors evermesh's own conformance runner
/// (`tools/conformance/src/kernel_target.rs::synth_blob` and
/// `tools/conformance/src/bin/generate.rs::synth_blob`) byte for byte — this
/// is the formula the vectors were generated from, not a magnetite
/// invention.
fn synth_blob(n_chunks: u64, last_chunk_len: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..n_chunks {
        let len = if i + 1 == n_chunks {
            last_chunk_len as usize
        } else {
            CHUNK_SIZE
        };
        out.resize(out.len() + len, (i % 251) as u8);
    }
    out
}

struct Vector {
    name: String,
    n_chunks: u64,
    last_chunk_len: u64,
    chunk_index: u64,
    proof: Vec<[u8; 32]>,
    root: [u8; 32],
    valid: bool,
}

fn load_vectors() -> Vec<Vector> {
    VECTOR_FILES
        .iter()
        .map(|(name, raw)| {
            let v: serde_json::Value =
                serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
            let n_chunks = v["n_chunks"].as_u64().expect("n_chunks");
            let last_chunk_len = v["last_chunk_len"].as_u64().expect("last_chunk_len");
            let chunk_index = v["chunk_index"].as_u64().expect("chunk_index");
            let proof: Vec<[u8; 32]> = v["proof_hex"]
                .as_array()
                .expect("proof_hex array")
                .iter()
                .map(|h| unhex32(h.as_str().expect("proof_hex entry is a string")))
                .collect();
            let root = unhex32(v["root_hex"].as_str().expect("root_hex"));
            let valid = v["valid"].as_bool().expect("valid");
            // Sanity: the vector's own `name` field agrees with the filename
            // this suite indexes it by, so a copy/rename mismatch surfaces
            // immediately rather than silently replaying the wrong fixture
            // under the wrong label.
            assert_eq!(
                v["name"].as_str().expect("name"),
                *name,
                "vector file's own `name` field disagrees with its filename"
            );
            Vector {
                name: (*name).to_string(),
                n_chunks,
                last_chunk_len,
                chunk_index,
                proof,
                root,
                valid,
            }
        })
        .collect()
}

#[test]
fn corpus_is_present_and_the_expected_size() {
    assert_eq!(
        VECTOR_FILES.len(),
        EXPECT_TOTAL,
        "vector count moved: this file is a frozen copy of evermesh's 14 \
         chunktree conformance vectors (see PROVENANCE.txt) — if it changed \
         on purpose, update EXPECT_TOTAL/EXPECT_VALID/EXPECT_INVALID in the \
         same commit. A corpus that silently shrank would otherwise let this \
         test report success having checked nothing."
    );
    let vs = load_vectors();
    let valid = vs.iter().filter(|v| v.valid).count();
    let invalid = vs.len() - valid;
    assert_eq!(
        (valid, invalid),
        (EXPECT_VALID, EXPECT_INVALID),
        "valid/invalid split moved: {valid} valid / {invalid} invalid"
    );
}

/// The actual replay: every one of the 14 vectors is reconstructed and fed
/// through `magnetite_seams::chunktree::verify_chunk`, and the outcome MUST
/// match the vector's own `valid` field exactly. This is the test that
/// proves the by-hand copy of evermesh's `EM-1` semantics agrees with
/// evermesh's own conformance ground truth, not just with this crate's own
/// restatement of it.
///
/// **If this test ever fails on a divergence, DO NOT weaken it, exclude the
/// vector, or change the expected counts to match. That failure IS the
/// finding this test exists to produce: a real disagreement between
/// `magnetite_seams::chunktree` and evermesh's `EM-1` profile.**
#[test]
fn every_vector_replays_to_its_expected_verdict() {
    let vectors = load_vectors();
    assert_eq!(
        vectors.len(),
        EXPECT_TOTAL,
        "examined {} vectors, expected exactly {EXPECT_TOTAL}",
        vectors.len()
    );

    let mut examined = 0usize;
    // Never incremented: see this file's header on why zero structural
    // exclusions is the expected, asserted shape here (unlike the CBOR
    // corpus replay, which does carve out an excluded bucket).
    let excluded = 0usize;
    let mut divergences: Vec<String> = Vec::new();

    for v in &vectors {
        let blob = synth_blob(v.n_chunks, v.last_chunk_len);
        let chunk_start = (v.chunk_index as usize) * CHUNK_SIZE;
        let chunk_end = if v.chunk_index + 1 == v.n_chunks {
            blob.len()
        } else {
            chunk_start + CHUNK_SIZE
        };

        let chunk = match blob.get(chunk_start..chunk_end) {
            Some(c) => c,
            // No such chunk exists in the synthesized blob at all (e.g. the
            // empty blob has zero chunks). A range proof for it cannot
            // possibly verify — correct only if the vector itself expects
            // `valid: false`; a vector claiming `valid: true` here would be
            // asking for something structurally impossible, which is a
            // divergence, not a pass.
            None => {
                examined += 1;
                if v.valid {
                    divergences.push(format!(
                        "{}: chunk_index {} has no bytes in the {}-chunk synthesized \
                         blob, but the vector expects valid: true",
                        v.name, v.chunk_index, v.n_chunks
                    ));
                }
                continue;
            }
        };

        let result = verify_chunk(
            &v.root,
            v.n_chunks as usize,
            v.chunk_index as usize,
            chunk,
            &v.proof,
        );
        examined += 1;
        match (result.is_ok(), v.valid) {
            (true, true) | (false, false) => {}
            (true, false) => divergences.push(format!(
                "{}: verify_chunk ACCEPTED a proof the vector marks invalid",
                v.name
            )),
            (false, true) => divergences.push(format!(
                "{}: verify_chunk REJECTED a proof the vector marks valid: {}",
                v.name,
                result.unwrap_err()
            )),
        }
    }

    assert!(
        divergences.is_empty(),
        "\n\n=== magnetite_seams::chunktree DIVERGES from evermesh's EM-1 conformance vectors ===\n\n{}\n",
        divergences.join("\n\n")
    );
    assert_eq!(
        examined, EXPECT_TOTAL,
        "examined {examined} vectors, expected exactly {EXPECT_TOTAL} — a vector was \
         skipped without being counted as excluded"
    );
    assert_eq!(
        excluded, EXPECT_EXCLUDED,
        "excluded {excluded} vectors as structurally inapplicable, expected exactly \
         {EXPECT_EXCLUDED} (see this file's header comment for why that should stay zero)"
    );
}

/// The replay must actually exercise interior nodes and multi-level proofs,
/// not just the trivial one-chunk (no-proof) case — otherwise a bug in
/// `reduce_level`'s pairing or in the odd-node promotion rule could hide
/// behind a suite that never has more than one leaf.
#[test]
fn replay_exercises_multi_level_proofs_not_just_trivial_cases() {
    let vectors = load_vectors();
    let with_nonempty_proof = vectors.iter().filter(|v| !v.proof.is_empty()).count();
    let max_proof_len = vectors.iter().map(|v| v.proof.len()).max().unwrap_or(0);
    assert!(
        with_nonempty_proof >= 8,
        "expected most vectors to carry a real sibling-hash proof; got {with_nonempty_proof}"
    );
    assert!(
        max_proof_len >= 3,
        "expected at least one vector needing a 3-level proof (the 5-chunk cases); \
         longest proof seen was {max_proof_len}"
    );
}
