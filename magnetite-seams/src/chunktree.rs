//! Chunk trees for verified range reads (backlog item A24).
//!
//! ADOPTED, BY DESIGN NOT BY DEPENDENCY, from evermesh's `EM-1` chunk-tree
//! profile (`evermesh/crates/evermesh-kernel/src/blob.rs`, spec
//! `001-kernel.md` §8, evermesh revision
//! `5436a56520c71ac3c7e97c3c1ee7e06de628f514`, 2026-07-30). `magnetite-seams`
//! has NO dependency, path or published, on evermesh — product
//! standalone-ness is owner directive #1 in `FANOUT-LOOP-STATE.md`. This
//! module copies evermesh's *semantics* by hand, exactly the way
//! `rotation.rs` (A8) copied evermesh's identity fork-resolution rule without
//! a dependency: same construction, same domain separation, same proof
//! shape, independently written and independently tested against a
//! byte-for-byte copy of evermesh's own conformance vectors
//! (`tests/vectors/evermesh-chunktree/`, replayed by
//! `tests/chunktree_conformance.rs`). Read the vectors directory's
//! `PROVENANCE.txt` for exact hashes and the copy chain.
//!
//! # Why this exists — A24
//!
//! [`crate::blobstore::BlobStore::get_range`] (A9) makes a byte range
//! *expressible* on every backend, but `magnetite-web-host`'s serving path
//! deliberately does **not** call it (commit `c5fcb96`): the only integrity
//! check available for a whole blob is a whole-blob BLAKE3 hash, and hashing
//! a slice proves nothing about a hash computed over the complete content.
//! Verifying a ranged read against that hash would require reading the
//! entire blob first, silently reintroducing the exact memory cost
//! `get_range` exists to avoid — see
//! `magnetite-web-host/tests/serving.rs::a_range_request_over_a_tampered_file_is_also_refused`,
//! which must keep failing a tampered ranged read exactly like a tampered
//! whole one.
//!
//! A **chunked Merkle hash** is the standard fix: split the blob into
//! fixed-size chunks, hash each into a binary tree, and publish only the
//! root. A verifier holding the root can then check any one chunk against it
//! in `O(log n)` hashes without touching the rest of the blob. evermesh
//! already has exactly this, specified (`spec/001-kernel.md` §8) and
//! conformance-tested (14 vectors under
//! `tools/conformance/vectors/chunktree/`) — A24 says plainly: do not design
//! a new chunk scheme, adopt evermesh's.
//!
//! # What was adopted (byte-for-byte semantics, profile `EM-1`)
//!
//! * Chunk size: exactly 1 MiB (`1 << 20`, [`CHUNK_SIZE`]); the final chunk
//!   MAY be shorter; the empty blob has zero chunks and no root.
//! * Leaf hash: `BLAKE3-256(0x00 ‖ chunk_bytes)` ([`leaf_hash`]).
//! * Interior hash: `BLAKE3-256(0x01 ‖ left ‖ right)` ([`node_hash`]). The
//!   `0x00`/`0x01` prefixes domain-separate leaves from interior nodes, so
//!   one can never be passed off as the other.
//! * Tree construction: pair nodes left to right; a level's unpaired final
//!   node is promoted unchanged (no duplication, no RFC-6962-style split at
//!   the largest power of two). A one-chunk blob's root is its own leaf
//!   hash.
//! * Proof shape: the sibling hash at every level that has one, bottom-up;
//!   combined by index parity (an even index combines as
//!   `node_hash(self, sibling)`, an odd index as `node_hash(sibling,
//!   self)`); a promoted (sibling-less) node contributes no proof entry at
//!   that level, and the verifier reconstructs which levels have one from
//!   `n_chunks` alone — never from a length prefix the proof itself carries.
//! * Root form: bare 32 bytes. (evermesh's second, DP-22 profile — DMTAP-PUB
//!   §22's distinctly-rooted variant of the same idea, multihash-framed and
//!   domain-tagged — is deliberately NOT adopted: magnetite has no DMTAP-PUB
//!   coupling, so there is only ever one profile here and no reason to carry
//!   its framing.)
//!
//! # What was deliberately left out
//!
//! * **Not wired into `magnetite-web-host`'s serving path.** This module is
//!   the primitive A24 asked for. Actually serving a verified range over
//!   HTTP needs a chunk root threaded through the package manifest
//!   (`package.rs`) and a proof-aware code path in `magnetite-web-host`,
//!   both materially bigger changes than fit alongside adopting the
//!   primitive itself — and rushing them risks weakening
//!   `a_range_request_over_a_tampered_file_is_also_refused`, which must not
//!   happen. Left for a follow-up task, exactly as A9's own doc comment
//!   anticipated ("a chunked / Merkle-tree hash scheme, for instance").
//! * **No streaming/reader-based builder.** evermesh's `ChunkTree::build`
//!   reads from a `std::io::Read` so a tree can be built without holding the
//!   whole blob in memory twice. This module only builds
//!   [`ChunkTree::from_bytes`]: every blob this crate currently chunks is
//!   already fully materialized by the time anything here would run, so a
//!   reader-based builder would add surface with no current caller. Worth
//!   adding the moment a streaming producer exists.
//! * **DP-22** (see above) — not applicable to this crate, not adopted.

use crate::error::{Result, SeamError};

/// Chunk size for chunk trees: 1 MiB, matching evermesh spec `001-kernel.md`
/// §8.
pub const CHUNK_SIZE: usize = 1 << 20;

/// Leaf hash: `BLAKE3-256(0x00 ‖ chunk_bytes)`.
pub fn leaf_hash(chunk: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x00]);
    hasher.update(chunk);
    *hasher.finalize().as_bytes()
}

/// Interior hash: `BLAKE3-256(0x01 ‖ left ‖ right)`.
pub fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x01]);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

/// Combine one level of a chunk tree into the next: pair nodes left to right
/// with [`node_hash`]; a level's unpaired final node is promoted unchanged.
fn reduce_level(level: &[[u8; 32]]) -> Vec<[u8; 32]> {
    let mut next = Vec::with_capacity(level.len().div_ceil(2));
    let mut i = 0;
    while i + 1 < level.len() {
        next.push(node_hash(&level[i], &level[i + 1]));
        i += 2;
    }
    if i < level.len() {
        next.push(level[i]);
    }
    next
}

/// The chunk tree of a blob: its leaf hashes plus total size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkTree {
    leaves: Vec<[u8; 32]>,
    size: u64,
}

impl ChunkTree {
    /// Build from an in-memory blob. The empty blob (zero bytes) yields the
    /// empty tree: zero chunks, no root.
    pub fn from_bytes(bytes: &[u8]) -> ChunkTree {
        let leaves = bytes.chunks(CHUNK_SIZE).map(leaf_hash).collect();
        ChunkTree {
            leaves,
            size: bytes.len() as u64,
        }
    }

    /// Merkle root (odd nodes promoted unchanged). `None` for the empty blob
    /// (zero chunks).
    pub fn root(&self) -> Option<[u8; 32]> {
        if self.leaves.is_empty() {
            return None;
        }
        let mut level = self.leaves.clone();
        while level.len() > 1 {
            level = reduce_level(&level);
        }
        Some(level[0])
    }

    /// Total blob size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Number of chunks (zero for the empty blob).
    pub fn n_chunks(&self) -> usize {
        self.leaves.len()
    }

    /// The leaf hashes, in chunk order.
    pub fn leaves(&self) -> &[[u8; 32]] {
        &self.leaves
    }

    /// Sibling path for chunk `index`, bottom-up. For a promoted
    /// (sibling-less) node at some level, no hash is emitted at that level —
    /// the verifier recomputes structure from `n_chunks`.
    pub fn prove(&self, index: usize) -> Result<Vec<[u8; 32]>> {
        if index >= self.leaves.len() {
            return Err(SeamError::ChunkProof("chunk index out of range".into()));
        }
        let mut proof = Vec::new();
        let mut level = self.leaves.clone();
        let mut cur = index;
        while level.len() > 1 {
            let sibling = cur ^ 1;
            if sibling < level.len() {
                proof.push(level[sibling]);
            }
            cur /= 2;
            level = reduce_level(&level);
        }
        Ok(proof)
    }
}

/// Verify one chunk against a root, given the blob's total chunk count and
/// the sibling path from [`ChunkTree::prove`]. Recomputes the tree structure
/// from `n_chunks`: at each level, if the current node index has a sibling
/// (`sibling_index < level_len`), consumes one proof hash (ordered
/// left/right by index parity); if it is the promoted last node, consumes
/// nothing.
pub fn verify_chunk(
    root: &[u8; 32],
    n_chunks: usize,
    index: usize,
    chunk: &[u8],
    proof: &[[u8; 32]],
) -> Result<()> {
    if index >= n_chunks {
        return Err(SeamError::ChunkProof("chunk index out of range".into()));
    }
    let mut node = leaf_hash(chunk);
    let mut level_len = n_chunks;
    let mut cur = index;
    let mut used = 0usize;
    while level_len > 1 {
        let sibling = cur ^ 1;
        if sibling < level_len {
            let sib = *proof
                .get(used)
                .ok_or_else(|| SeamError::ChunkProof("proof too short".into()))?;
            used += 1;
            node = if cur.is_multiple_of(2) {
                node_hash(&node, &sib)
            } else {
                node_hash(&sib, &node)
            };
        }
        cur /= 2;
        level_len = level_len.div_ceil(2);
    }
    if used != proof.len() {
        return Err(SeamError::ChunkProof("proof too long".into()));
    }
    if node != *root {
        return Err(SeamError::ChunkProof("root mismatch".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a blob of `n_chunks` chunks, all full [`CHUNK_SIZE`] except the
    /// last, which is `last_len` bytes. Each chunk is filled with a distinct
    /// repeating byte (its index) so that chunks, and thus their leaf
    /// hashes, are pairwise distinct.
    fn make_blob(n_chunks: usize, last_len: usize) -> Vec<u8> {
        assert!(n_chunks > 0);
        assert!(last_len > 0 && last_len <= CHUNK_SIZE);
        let mut out = Vec::with_capacity((n_chunks - 1) * CHUNK_SIZE + last_len);
        for i in 0..n_chunks {
            let len = if i + 1 == n_chunks {
                last_len
            } else {
                CHUNK_SIZE
            };
            out.extend(std::iter::repeat_n(i as u8, len));
        }
        out
    }

    /// Independent reference implementation of the root reduction, used to
    /// cross-check `ChunkTree::root` / `reduce_level`.
    fn reference_root(leaves: &[[u8; 32]]) -> Option<[u8; 32]> {
        if leaves.is_empty() {
            return None;
        }
        let mut level: Vec<[u8; 32]> = leaves.to_vec();
        while level.len() > 1 {
            let mut next = Vec::new();
            for pair in level.chunks(2) {
                if pair.len() == 2 {
                    next.push(node_hash(&pair[0], &pair[1]));
                } else {
                    next.push(pair[0]);
                }
            }
            level = next;
        }
        Some(level[0])
    }

    #[test]
    fn empty_blob_has_no_chunks_and_no_root() {
        let tree = ChunkTree::from_bytes(&[]);
        assert_eq!(tree.n_chunks(), 0);
        assert_eq!(tree.size(), 0);
        assert_eq!(tree.root(), None);
        assert!(tree.leaves().is_empty());
    }

    #[test]
    fn empty_blob_verify_chunk_rejects_any_index() {
        let root = [0u8; 32];
        assert!(verify_chunk(&root, 0, 0, b"x", &[]).is_err());
    }

    #[test]
    fn one_chunk_root_is_the_leaf_hash() {
        let bytes = make_blob(1, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        assert_eq!(tree.n_chunks(), 1);
        assert_eq!(tree.size(), CHUNK_SIZE as u64);
        assert_eq!(tree.root(), Some(leaf_hash(&bytes)));
    }

    #[test]
    fn manual_three_chunk_root_shape() {
        // root = node_hash(node_hash(l0, l1), l2): l2 is promoted unchanged
        // at level 0, then combined with the level-1 pair.
        let bytes = make_blob(3, CHUNK_SIZE);
        let l0 = leaf_hash(&bytes[0..CHUNK_SIZE]);
        let l1 = leaf_hash(&bytes[CHUNK_SIZE..2 * CHUNK_SIZE]);
        let l2 = leaf_hash(&bytes[2 * CHUNK_SIZE..3 * CHUNK_SIZE]);
        let expected = node_hash(&node_hash(&l0, &l1), &l2);

        let tree = ChunkTree::from_bytes(&bytes);
        assert_eq!(tree.leaves(), &[l0, l1, l2]);
        assert_eq!(tree.root(), Some(expected));
    }

    #[test]
    fn known_structure_against_reference_for_several_counts() {
        for &n in &[1usize, 2, 3, 4, 5, 7] {
            let bytes = make_blob(n, CHUNK_SIZE);
            let tree = ChunkTree::from_bytes(&bytes);
            assert_eq!(tree.n_chunks(), n, "n_chunks for count {n}");
            let expected = reference_root(tree.leaves());
            assert_eq!(tree.root(), expected, "root mismatch for count {n}");
        }
    }

    #[test]
    fn boundary_exact_chunk_size_is_one_chunk() {
        let bytes = vec![0xabu8; CHUNK_SIZE];
        let tree = ChunkTree::from_bytes(&bytes);
        assert_eq!(tree.n_chunks(), 1);
        assert_eq!(tree.size(), CHUNK_SIZE as u64);
    }

    #[test]
    fn boundary_chunk_size_plus_one_is_two_chunks() {
        let bytes = vec![0xcdu8; CHUNK_SIZE + 1];
        let tree = ChunkTree::from_bytes(&bytes);
        assert_eq!(tree.n_chunks(), 2);
        assert_eq!(tree.size(), (CHUNK_SIZE + 1) as u64);
        assert_eq!(tree.leaves()[1], leaf_hash(&bytes[CHUNK_SIZE..]));
    }

    #[test]
    fn prove_and_verify_every_index_for_several_counts() {
        for &n in &[1usize, 2, 3, 4, 5] {
            let bytes = make_blob(n, CHUNK_SIZE);
            let tree = ChunkTree::from_bytes(&bytes);
            let root = tree.root().expect("non-empty tree has a root");
            for index in 0..n {
                let chunk_start = index * CHUNK_SIZE;
                let chunk = &bytes[chunk_start..chunk_start + CHUNK_SIZE];
                let proof = tree.prove(index).expect("valid index");
                verify_chunk(&root, n, index, chunk, &proof)
                    .unwrap_or_else(|e| panic!("verify failed for n={n} index={index}: {e:?}"));
            }
        }
    }

    #[test]
    fn prove_out_of_range_index_errors() {
        let bytes = make_blob(3, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        assert!(tree.prove(3).is_err());
        assert!(tree.prove(usize::MAX).is_err());
    }

    #[test]
    fn verify_rejects_wrong_chunk_bytes() {
        let bytes = make_blob(4, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let root = tree.root().unwrap();
        let proof = tree.prove(1).unwrap();
        let mut wrong_chunk = bytes[CHUNK_SIZE..2 * CHUNK_SIZE].to_vec();
        wrong_chunk[0] ^= 0xff;
        assert!(verify_chunk(&root, 4, 1, &wrong_chunk, &proof).is_err());
    }

    #[test]
    fn verify_rejects_wrong_index() {
        let bytes = make_blob(4, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let root = tree.root().unwrap();
        let proof = tree.prove(1).unwrap();
        let chunk = &bytes[CHUNK_SIZE..2 * CHUNK_SIZE];
        // Same chunk bytes and proof, but claimed at a different index.
        assert!(verify_chunk(&root, 4, 2, chunk, &proof).is_err());
    }

    #[test]
    fn verify_rejects_truncated_proof() {
        let bytes = make_blob(5, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let root = tree.root().unwrap();
        let index = 3;
        let proof = tree.prove(index).unwrap();
        assert!(
            !proof.is_empty(),
            "5-chunk tree should need at least one sibling"
        );
        let truncated = &proof[..proof.len() - 1];
        let chunk_start = index * CHUNK_SIZE;
        let chunk = &bytes[chunk_start..chunk_start + CHUNK_SIZE];
        assert!(verify_chunk(&root, 5, index, chunk, truncated).is_err());
    }

    #[test]
    fn verify_rejects_over_long_proof() {
        let bytes = make_blob(5, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let root = tree.root().unwrap();
        let index = 3;
        let mut proof = tree.prove(index).unwrap();
        proof.push([0x42; 32]);
        let chunk_start = index * CHUNK_SIZE;
        let chunk = &bytes[chunk_start..chunk_start + CHUNK_SIZE];
        assert!(verify_chunk(&root, 5, index, chunk, &proof).is_err());
    }

    #[test]
    fn verify_rejects_swapped_sibling_order() {
        let bytes = make_blob(4, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let root = tree.root().unwrap();
        let index = 0;
        let mut proof = tree.prove(index).unwrap();
        assert!(
            proof.len() >= 2,
            "4-chunk tree at index 0 has a two-level path"
        );
        proof.swap(0, 1);
        let chunk = &bytes[0..CHUNK_SIZE];
        assert!(verify_chunk(&root, 4, index, chunk, &proof).is_err());
    }

    #[test]
    fn verify_rejects_wrong_root() {
        let bytes = make_blob(3, CHUNK_SIZE);
        let tree = ChunkTree::from_bytes(&bytes);
        let wrong_root = [0x99; 32];
        let index = 0;
        let proof = tree.prove(index).unwrap();
        let chunk = &bytes[0..CHUNK_SIZE];
        assert!(verify_chunk(&wrong_root, 3, index, chunk, &proof).is_err());
    }

    #[test]
    fn leaf_and_node_hash_are_domain_separated() {
        let a = [0x11; 32];
        let b = [0x22; 32];
        let leaf = leaf_hash(&[0x11; 32]);
        let node = node_hash(&a, &b);
        assert_ne!(leaf, node);
    }
}
