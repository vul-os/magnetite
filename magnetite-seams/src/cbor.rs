//! Canonical **integer-keyed deterministic CBOR** — the signed/content-addressed
//! wire format (`ALIGNMENT.md` §3, "One wire-format boundary to get right").
//!
//! # Why this module exists rather than `serde` + `ciborium`
//!
//! Magnetite is JSON throughout, and JSON is fine for the `mag_*` host↔guest ABI
//! (neither signed nor on the wire). It is **not** fine for anything that gets
//! signed or content-addressed. A signature is a statement about *bytes*; if two
//! honest implementations can serialize the same object to different bytes, the
//! signature says nothing and the content address is not an address. serde /
//! `ciborium` encodings are text-keyed (struct field names), field-order
//! dependent, and free to choose non-shortest-form heads — a second implementer
//! writing to a written spec would not reproduce them.
//!
//! So signed objects are encoded here, by hand, as integer-keyed maps under
//! RFC 8949 Core Deterministic Encoding. Field *numbers* are the schema; Rust
//! field names are an implementation detail.
//!
//! # Relationship to `kotva-core::cbor`
//!
//! This is deliberately the **same encoding** as
//! `kotva-core/src/cbor.rs` (DMTAP spec §18.1.1 / §18.1.2), reimplemented here
//! rather than depended on. Two reasons:
//!
//! 1. `magnetite-seams` depends on no sibling repo by path — see this crate's
//!    `Cargo.toml` for the empirical reason (an optional `../../patala` path dep
//!    broke `cargo test` non-deterministically because Cargo must resolve
//!    optional path deps to build a lockfile). The kotva binding is a
//!    tag-pinned git dependency to be added deliberately, in one place, when
//!    that binding lands — not smuggled in under this module.
//! 2. ~~Having a second independent implementation of the encoding is exactly
//!    the thing the DMTAP spec asks for: if the two disagree, one of them is
//!    wrong, and the disagreement is findable.~~ **Superseded — see the
//!    TEMPORARY STATE notice below.** Reason 1 stands; reason 2 does not.
//!
//! **Intended binding.** When kotva-core is linked (`kotva-core = { git = ...,
//! tag = "core-v0.2.0" }`), [`Cv`] maps 1:1 onto `kotva_core::cbor::Cv` minus
//! the `TextMap` variant (kotva admits text keys in exactly one place,
//! `Headers.ext` §18.3.6; magnetite needs none, so this subset forbids them
//! outright and is strictly stricter). [`encode`] and [`decode`] are then
//! replaceable by kotva's with no change to the byte stream. Until that happens,
//! nothing here reaches outside crates.io.
//!
//! # ⚠ TEMPORARY STATE — this module is a known SOVEREIGNTY.md §3.5 violation
//!
//! **Do not read the section above as a settled design.** It is the reasoning
//! this module was written under, kept because it explains the code, but it has
//! been overtaken. Recorded per `ALIGNMENT.md` §9 item 1 and
//! `docs/cross-repo-backlog.md` **A2**:
//!
//! * `kotva/substrate/SOVEREIGNTY.md` §3.5 requires consumers to run the
//!   **shared compiled algebra**, not merely an algorithm that agrees
//!   byte-for-byte. Under that rule a byte-identical reimplementation is *still*
//!   a violation, so reason 2 above — "a second independent implementation is a
//!   feature" — is no longer the house position. Two codecs that agree today can
//!   drift tomorrow, and nothing in either tree would notice.
//! * There are in fact **three** implementations in the family, not two:
//!   this one, `kotva-core/src/cbor.rs`, and `evermesh-kernel/src/codec.rs` —
//!   the last being the most mature (189 conformance vectors replayed across
//!   three runtimes).
//!
//! **Update 2026-07-30 — a partial cross-check now exists, and it is clean.**
//! `tests/evermesh_conformance.rs` replays the same frozen corpus kotva-cbor
//! carries (itself a mechanically-verified byte-for-byte extraction of
//! evermesh's 189 vectors) against this module's own `decode`/`encode`. Of the
//! 153 `accept` vectors, only **9** apply — this type is strictly narrower
//! than `kotva_cbor::Value` (no `Nint`, no `Null`, `u64`-only map keys), so the
//! other 144 use shapes `Cv` cannot represent at all, which is a scope gap,
//! not a codec disagreement. On the 9 that do apply, and on all 30 `reject`
//! vectors, **zero divergence was found**: every applicable accept vector
//! decodes and re-encodes to itself, and every reject vector is refused. That
//! is real evidence on a real subset — it is **not** the full 189-vector
//! cross-validation the exit plan below still calls for, because most of the
//! corpus is structurally out of reach for this narrower type.
//!
//! **Why it is being merged as-is anyway, deliberately.** Collapsing onto one
//! codec is blocked on `kotva-cbor` being published to **crates.io** — a
//! crates.io dependency is the only form that satisfies both §3.5 and this
//! crate's own no-git-dependency rule (reason 1, which still holds). That
//! publication has not happened; `kotva-cbor`'s extraction is itself unfinished
//! (backlog C1) and another agent holds the kotva side. Rewriting this module
//! against an unpublished crate now would trade a recorded violation for a
//! broken build.
//!
//! **The exit, in order:** publish `kotva-cbor` (C3) → cross-validate all three
//! codecs against evermesh's 189 vectors on the shared subset, in
//! `magnetite-kotva`, the one crate legitimately depending on more than one (C2)
//! → delete this module in favour of the published crate. Until every step
//! lands, treat full byte-identity with kotva as **asserted on 9/153 vectors,
//! proven; on the rest, still unproven** — this module remains a §3.5
//! violation regardless, because §3.5 requires running the shared compiled
//! algebra, not merely agreeing with it where the two domains overlap.
//!
//! # Rules enforced (both directions)
//!
//! 1. Shortest-form integers, lengths and counts. No indefinite-length items.
//! 2. Map keys sorted ascending **by their encoded bytes** (for the small
//!    unsigned keys used here, identical to numeric order).
//! 3. No duplicate keys.
//! 4. No floating-point values anywhere — money is integer smallest-units and
//!    hashes are byte strings, so nothing needs a float.
//! 5. No tags, no `undefined`, no `null`, no negative integers. An absent
//!    optional is **omitted from the map**, never present as `null`.
//! 6. Exactly one top-level item; trailing bytes are an error.
//!
//! The decoder accepts *only* canonical bytes, so `encode(decode(b)) == b` for
//! every `b` it accepts. That is the property signature verification needs: a
//! received object can be re-encoded and the signature checked over the result
//! without the sender being able to smuggle in a second valid encoding.
//!
//! [`decode`] is a **strict** parser over untrusted bytes and fails closed on
//! the first deviation; it never normalizes-and-accepts.

/// A canonical CBOR value, restricted to the subset magnetite signs.
///
/// Deliberately missing: floats, negative integers, `null`, `undefined`, tags,
/// and text-keyed maps. If a schema seems to need one of those, the schema is
/// wrong for a signed object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cv {
    /// Unsigned integer (major type 0).
    U64(u64),
    /// Byte string (major type 2) — hashes, keys, signatures.
    Bytes(Vec<u8>),
    /// UTF-8 text string (major type 3) — paths, currency labels, role names.
    Text(String),
    /// Boolean (major type 7, `0xf4` / `0xf5`).
    Bool(bool),
    /// Definite-length array (major type 4).
    Array(Vec<Cv>),
    /// Integer-keyed map (major type 5) — the object encoding.
    Map(Vec<(u64, Cv)>),
}

/// A deviation from the canonical encoding, or a schema violation found while
/// reading one. Every variant means *refuse*, never *repair*.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CborError {
    /// Truncated, or otherwise not parseable as CBOR at all.
    #[error("malformed CBOR")]
    Malformed,
    /// A head encoded longer than necessary (e.g. `0x18 0x0a` for 10).
    #[error("non-shortest-form integer/length encoding")]
    NonShortestForm,
    /// An indefinite-length item or a `break` code.
    #[error("indefinite-length item is forbidden")]
    IndefiniteLength,
    /// Map keys not in strictly ascending encoded-byte order.
    #[error("map keys are not in strictly ascending encoded-byte order")]
    MapKeyOrder,
    /// More than one top-level item.
    #[error("trailing bytes after the top-level CBOR item")]
    TrailingData,
    /// A float, `NaN` or infinity appeared.
    #[error("floating-point value is forbidden")]
    FloatPresent,
    /// A CBOR `null` appeared. Absent optionals are omitted, not nulled.
    #[error("CBOR null is forbidden — absent optionals are omitted")]
    NullPresent,
    /// A tag, `undefined`, or an unassigned simple value appeared.
    #[error("CBOR tag / undefined / simple value is forbidden")]
    TagOrUndefined,
    /// The same map key twice.
    #[error("duplicate map key {0}")]
    DuplicateKey(u64),
    /// A text key, or a mix of key types, in a map.
    #[error("map key is not an unsigned integer")]
    NonIntegerKey,
    /// A negative integer.
    #[error("negative integer is forbidden")]
    NegativeInteger,
    /// A field held the wrong CBOR type for its schema slot.
    #[error("unexpected CBOR type for field {0}")]
    TypeMismatch(u64),
    /// A key not in the schema. **Signed objects fail closed on unknown keys**:
    /// silently ignoring one would let a signer include a field the verifier
    /// cannot see.
    #[error("unknown key {0} in a signed object")]
    UnknownKey(u64),
    /// A required field was absent.
    #[error("missing required key {0}")]
    MissingKey(u64),
    /// An enum discriminator outside the schema's range.
    #[error("unknown enum discriminator {1} for field {0}")]
    UnknownDiscriminant(u64, u64),
    /// A byte string whose length is fixed by the schema had the wrong length.
    #[error("field {0}: expected {1} bytes, got {2}")]
    WrongLength(u64, usize, usize),
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Write a CBOR head: 3-bit major type plus the shortest form of `arg`.
fn write_head(out: &mut Vec<u8>, major: u8, arg: u64) {
    let m = major << 5;
    if arg < 24 {
        out.push(m | arg as u8);
    } else if arg <= u8::MAX as u64 {
        out.push(m | 24);
        out.push(arg as u8);
    } else if arg <= u16::MAX as u64 {
        out.push(m | 25);
        out.extend_from_slice(&(arg as u16).to_be_bytes());
    } else if arg <= u32::MAX as u64 {
        out.push(m | 26);
        out.extend_from_slice(&(arg as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&arg.to_be_bytes());
    }
}

/// Encode a [`Cv`] as canonical deterministic CBOR.
///
/// Infallible: `Cv` cannot represent a forbidden value, and map keys are sorted
/// here rather than trusted from the caller — so an out-of-order [`Cv::Map`]
/// still encodes canonically.
pub fn encode(v: &Cv) -> Vec<u8> {
    let mut out = Vec::new();
    enc(v, &mut out);
    out
}

fn enc(v: &Cv, out: &mut Vec<u8>) {
    match v {
        Cv::U64(n) => write_head(out, 0, *n),
        Cv::Bytes(b) => {
            write_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Cv::Text(s) => {
            write_head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Cv::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Cv::Array(a) => {
            write_head(out, 4, a.len() as u64);
            for e in a {
                enc(e, out);
            }
        }
        Cv::Map(m) => {
            // Sort by encoded key bytes (rule 2). Done here, not assumed of the
            // caller, so building a map in schema-readable order is safe.
            let mut items: Vec<(Vec<u8>, &Cv)> = m
                .iter()
                .map(|(k, val)| {
                    let mut kb = Vec::new();
                    write_head(&mut kb, 0, *k);
                    (kb, val)
                })
                .collect();
            items.sort_by(|a, b| a.0.cmp(&b.0));
            write_head(out, 5, items.len() as u64);
            for (kb, val) in items {
                out.extend_from_slice(&kb);
                enc(val, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Nesting depth ceiling. Each nested container costs a native stack frame, so
/// an unbounded decoder lets a few KB of `0x81` (nested one-element arrays)
/// overflow the stack and abort the process — reachable from any parse of
/// untrusted bytes. Package manifests are shallow (object → field → array →
/// array → scalar, four levels), so 32 is far above anything real and far below
/// the overflow threshold. Exceeding it fails closed instead of panicking.
const MAX_NESTING_DEPTH: u32 = 32;

/// Parse **canonical** CBOR, failing closed on any deviation.
///
/// This rejects input a lenient CBOR library would happily accept and
/// re-serialize differently — which is the whole point: a verifier must not be
/// able to be handed a second, differently-encoded form of a signed object.
pub fn decode(bytes: &[u8]) -> Result<Cv, CborError> {
    let mut d = Decoder { b: bytes, pos: 0 };
    let cv = d.value(0)?;
    if d.pos != bytes.len() {
        return Err(CborError::TrailingData);
    }
    Ok(cv)
}

struct Decoder<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn byte(&mut self) -> Result<u8, CborError> {
        let b = *self.b.get(self.pos).ok_or(CborError::Malformed)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CborError> {
        let end = self.pos.checked_add(n).ok_or(CborError::Malformed)?;
        let s = self.b.get(self.pos..end).ok_or(CborError::Malformed)?;
        self.pos = end;
        Ok(s)
    }

    /// Read a head argument, enforcing shortest form and rejecting
    /// indefinite-length / reserved additional-info values.
    fn argument(&mut self, ai: u8) -> Result<u64, CborError> {
        match ai {
            0..=23 => Ok(ai as u64),
            24 => {
                let v = self.byte()? as u64;
                if v < 24 {
                    return Err(CborError::NonShortestForm);
                }
                Ok(v)
            }
            25 => {
                let b = self.take(2)?;
                let v = u16::from_be_bytes([b[0], b[1]]) as u64;
                if v <= u8::MAX as u64 {
                    return Err(CborError::NonShortestForm);
                }
                Ok(v)
            }
            26 => {
                let b = self.take(4)?;
                let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64;
                if v <= u16::MAX as u64 {
                    return Err(CborError::NonShortestForm);
                }
                Ok(v)
            }
            27 => {
                let b = self.take(8)?;
                let v = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
                if v <= u32::MAX as u64 {
                    return Err(CborError::NonShortestForm);
                }
                Ok(v)
            }
            28..=30 => Err(CborError::Malformed),  // reserved
            _ => Err(CborError::IndefiniteLength), // 31 = indefinite / break
        }
    }

    fn value(&mut self, depth: u32) -> Result<Cv, CborError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(CborError::Malformed);
        }
        let ib = self.byte()?;
        let major = ib >> 5;
        let ai = ib & 0x1f;
        match major {
            0 => Ok(Cv::U64(self.argument(ai)?)),
            1 => {
                // Negative integer: consume (and length-check) the argument so
                // the position stays meaningful, then refuse the value.
                let _ = self.argument(ai)?;
                Err(CborError::NegativeInteger)
            }
            2 => {
                let n = self.argument(ai)? as usize;
                Ok(Cv::Bytes(self.take(n)?.to_vec()))
            }
            3 => {
                let n = self.argument(ai)? as usize;
                let s = self.take(n)?;
                let s = std::str::from_utf8(s).map_err(|_| CborError::Malformed)?;
                Ok(Cv::Text(s.to_owned()))
            }
            4 => {
                let n = self.argument(ai)?;
                // Do not pre-allocate on an attacker-chosen count: a 5-byte
                // head can claim 2^32 elements.
                let mut out = Vec::new();
                for _ in 0..n {
                    out.push(self.value(depth + 1)?);
                }
                Ok(Cv::Array(out))
            }
            5 => self.map(ai, depth),
            6 => {
                let _ = self.argument(ai)?;
                Err(CborError::TagOrUndefined)
            }
            _ => self.simple(ai),
        }
    }

    fn map(&mut self, ai: u8, depth: u32) -> Result<Cv, CborError> {
        let n = self.argument(ai)?;
        let mut out: Vec<(u64, Cv)> = Vec::new();
        let mut prev: Option<Vec<u8>> = None;
        for _ in 0..n {
            let key_start = self.pos;
            let key = self.value(depth + 1)?;
            let key_bytes = self.b[key_start..self.pos].to_vec();
            let k = match key {
                Cv::U64(k) => k,
                _ => return Err(CborError::NonIntegerKey),
            };
            if let Some(p) = &prev {
                match key_bytes.as_slice().cmp(p.as_slice()) {
                    std::cmp::Ordering::Greater => {}
                    std::cmp::Ordering::Equal => return Err(CborError::DuplicateKey(k)),
                    std::cmp::Ordering::Less => return Err(CborError::MapKeyOrder),
                }
            }
            prev = Some(key_bytes);
            let v = self.value(depth + 1)?;
            out.push((k, v));
        }
        Ok(Cv::Map(out))
    }

    /// Major type 7: only `false`/`true` are admitted.
    fn simple(&mut self, ai: u8) -> Result<Cv, CborError> {
        match ai {
            20 => Ok(Cv::Bool(false)),
            21 => Ok(Cv::Bool(true)),
            22 => Err(CborError::NullPresent),
            25 => {
                let _ = self.take(2)?;
                Err(CborError::FloatPresent)
            }
            26 => {
                let _ = self.take(4)?;
                Err(CborError::FloatPresent)
            }
            27 => {
                let _ = self.take(8)?;
                Err(CborError::FloatPresent)
            }
            31 => Err(CborError::IndefiniteLength),
            _ => Err(CborError::TagOrUndefined),
        }
    }
}

// ---------------------------------------------------------------------------
// Schema-reading helpers
// ---------------------------------------------------------------------------

/// A decoded integer-keyed map, read field by field with unknown keys refused.
///
/// Built for reading **signed** objects, so the discipline is deliberately
/// unforgiving: [`MapReader::finish`] errors if any key was not taken. Ignoring
/// an unknown key would mean the signer put something in the signed bytes that
/// the verifier never looked at.
pub struct MapReader {
    items: Vec<(u64, Option<Cv>)>,
}

impl MapReader {
    /// Wrap a decoded value that must be a map.
    pub fn new(v: Cv) -> Result<Self, CborError> {
        match v {
            Cv::Map(m) => Ok(Self {
                items: m.into_iter().map(|(k, v)| (k, Some(v))).collect(),
            }),
            _ => Err(CborError::TypeMismatch(0)),
        }
    }

    /// Take the value at `key`, if present.
    pub fn take(&mut self, key: u64) -> Option<Cv> {
        self.items
            .iter_mut()
            .find(|(k, v)| *k == key && v.is_some())
            .and_then(|(_, v)| v.take())
    }

    /// Take a required value at `key`.
    pub fn require(&mut self, key: u64) -> Result<Cv, CborError> {
        self.take(key).ok_or(CborError::MissingKey(key))
    }

    /// Take a required `u64`.
    pub fn u64(&mut self, key: u64) -> Result<u64, CborError> {
        match self.require(key)? {
            Cv::U64(n) => Ok(n),
            _ => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Take an optional `u64`.
    pub fn opt_u64(&mut self, key: u64) -> Result<Option<u64>, CborError> {
        match self.take(key) {
            None => Ok(None),
            Some(Cv::U64(n)) => Ok(Some(n)),
            Some(_) => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Take a required text string.
    pub fn text(&mut self, key: u64) -> Result<String, CborError> {
        match self.require(key)? {
            Cv::Text(s) => Ok(s),
            _ => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Take an optional text string.
    pub fn opt_text(&mut self, key: u64) -> Result<Option<String>, CborError> {
        match self.take(key) {
            None => Ok(None),
            Some(Cv::Text(s)) => Ok(Some(s)),
            Some(_) => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Take a required byte string of exactly `n` bytes.
    pub fn bytes_exact(&mut self, key: u64, n: usize) -> Result<Vec<u8>, CborError> {
        match self.require(key)? {
            Cv::Bytes(b) if b.len() == n => Ok(b),
            Cv::Bytes(b) => Err(CborError::WrongLength(key, n, b.len())),
            _ => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Take a required array.
    pub fn array(&mut self, key: u64) -> Result<Vec<Cv>, CborError> {
        match self.require(key)? {
            Cv::Array(a) => Ok(a),
            _ => Err(CborError::TypeMismatch(key)),
        }
    }

    /// Refuse leftovers. Call this at the end of every signed-object reader.
    pub fn finish(self) -> Result<(), CborError> {
        for (k, v) in self.items {
            if v.is_some() {
                return Err(CborError::UnknownKey(k));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_form_heads() {
        assert_eq!(encode(&Cv::U64(0)), vec![0x00]);
        assert_eq!(encode(&Cv::U64(23)), vec![0x17]);
        assert_eq!(encode(&Cv::U64(24)), vec![0x18, 0x18]);
        assert_eq!(encode(&Cv::U64(255)), vec![0x18, 0xff]);
        assert_eq!(encode(&Cv::U64(256)), vec![0x19, 0x01, 0x00]);
        assert_eq!(encode(&Cv::U64(65_536)), vec![0x1a, 0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn non_shortest_form_is_refused() {
        // 10 encoded in a 1-byte-argument head instead of inline.
        assert_eq!(decode(&[0x18, 0x0a]), Err(CborError::NonShortestForm));
        // 200 encoded in a 2-byte-argument head.
        assert_eq!(decode(&[0x19, 0x00, 0xc8]), Err(CborError::NonShortestForm));
    }

    #[test]
    fn map_keys_are_sorted_on_encode_regardless_of_input_order() {
        let a = Cv::Map(vec![(3, Cv::U64(30)), (1, Cv::U64(10)), (2, Cv::U64(20))]);
        let b = Cv::Map(vec![(1, Cv::U64(10)), (2, Cv::U64(20)), (3, Cv::U64(30))]);
        assert_eq!(encode(&a), encode(&b));
        assert_eq!(
            encode(&b),
            vec![0xa3, 0x01, 0x0a, 0x02, 0x14, 0x03, 0x18, 0x1e]
        );
    }

    #[test]
    fn out_of_order_and_duplicate_keys_are_refused_on_decode() {
        // {2: 0, 1: 0} — descending.
        assert_eq!(
            decode(&[0xa2, 0x02, 0x00, 0x01, 0x00]),
            Err(CborError::MapKeyOrder)
        );
        // {1: 0, 1: 0} — duplicate.
        assert_eq!(
            decode(&[0xa2, 0x01, 0x00, 0x01, 0x00]),
            Err(CborError::DuplicateKey(1))
        );
    }

    #[test]
    fn floats_null_tags_negatives_and_indefinite_are_all_refused() {
        assert_eq!(decode(&[0xf9, 0x00, 0x00]), Err(CborError::FloatPresent)); // f16 0.0
        assert_eq!(
            decode(&[0xfb, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(CborError::FloatPresent)
        ); // f64
        assert_eq!(decode(&[0xf6]), Err(CborError::NullPresent));
        assert_eq!(decode(&[0xf7]), Err(CborError::TagOrUndefined)); // undefined
        assert_eq!(decode(&[0xc0, 0x00]), Err(CborError::TagOrUndefined)); // tag(0)
        assert_eq!(decode(&[0x20]), Err(CborError::NegativeInteger)); // -1
        assert_eq!(decode(&[0x9f, 0xff]), Err(CborError::IndefiniteLength));
        assert_eq!(decode(&[0x5f, 0xff]), Err(CborError::IndefiniteLength));
    }

    #[test]
    fn text_keys_are_refused() {
        // {"a": 0}
        assert_eq!(
            decode(&[0xa1, 0x61, 0x61, 0x00]),
            Err(CborError::NonIntegerKey)
        );
    }

    #[test]
    fn trailing_bytes_are_refused() {
        assert_eq!(decode(&[0x00, 0x00]), Err(CborError::TrailingData));
    }

    #[test]
    fn deep_nesting_fails_closed_instead_of_overflowing() {
        // 10_000 nested one-element arrays.
        let mut b = vec![0x81; 10_000];
        b.push(0x00);
        assert_eq!(decode(&b), Err(CborError::Malformed));
    }

    #[test]
    fn a_huge_claimed_array_count_does_not_allocate() {
        // Array head claiming 2^32-ish elements with no body: must error on the
        // first missing element, not try to reserve.
        assert_eq!(
            decode(&[0x9a, 0xff, 0xff, 0xff, 0xff]),
            Err(CborError::Malformed)
        );
    }

    #[test]
    fn reencoding_a_decoded_value_reproduces_the_bytes() {
        let v = Cv::Map(vec![
            (1, Cv::U64(1)),
            (2, Cv::Text("index.html".into())),
            (3, Cv::Bytes(vec![0xAB; 32])),
            (4, Cv::Array(vec![Cv::Bool(true), Cv::U64(70_000)])),
        ]);
        let bytes = encode(&v);
        let back = decode(&bytes).unwrap();
        assert_eq!(back, v);
        assert_eq!(encode(&back), bytes);
    }

    #[test]
    fn map_reader_refuses_unknown_keys() {
        let v = decode(&encode(&Cv::Map(vec![(1, Cv::U64(7)), (9, Cv::U64(0))]))).unwrap();
        let mut r = MapReader::new(v).unwrap();
        assert_eq!(r.u64(1).unwrap(), 7);
        assert_eq!(r.finish(), Err(CborError::UnknownKey(9)));
    }

    #[test]
    fn map_reader_requires_required_keys() {
        let v = decode(&encode(&Cv::Map(vec![(1, Cv::U64(7))]))).unwrap();
        let mut r = MapReader::new(v).unwrap();
        assert_eq!(r.u64(2), Err(CborError::MissingKey(2)));
    }
}
