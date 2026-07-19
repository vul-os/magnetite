//! Seam §3.2 — a **second** `Naming` provider: word-based key-names.
//!
//! This module exists to prove the [`Naming`](crate::naming::Naming) seam is
//! genuinely pluggable — that consuming code is written against the trait and is
//! not accidentally hardwired to the default implementation.
//!
//! # What this is
//!
//! [`KeyNameNaming`] renders an Ed25519 public key as a sequence of words drawn
//! from a fixed, embedded 2048-word list (11 bits per word — see
//! [`wordlist`]). It is *zero-authority*: the name is derived purely from the
//! key, with no registry, no chain, and no server. An optional per-key domain
//! hint lets a UI show `<words>@example.org`; the hint is a **local hint only**,
//! never authoritative, exactly like [`HashNaming`](crate::naming::HashNaming)'s
//! alias table.
//!
//! # Compatibility — claims nothing
//!
//! Word-based key-names are a common idea (BIP-39 and various "zero-authority"
//! naming ladders use the same shape), but this encoding is **its own**: the
//! wordlist, bit packing, checksum, and separator here are not claimed to match
//! any other scheme, so names produced here will not match names another system
//! produces for the same key. The type is named for what it is
//! (`KeyNameNaming`) rather than after any protocol, precisely so no
//! unverifiable compatibility claim is smuggled into a type name.
//!
//! Any other naming provider implements the same
//! [`Naming`](crate::naming::Naming) trait alongside this one and nothing else
//! in the tree changes — that is the whole point of the seam.
//!
//! # Two name forms, and an honest word about bit budgets
//!
//! Eight words carry `8 × 11 = 88` bits. An Ed25519 public key is **256** bits.
//! An 8-word name therefore *cannot* be a lossless encoding of a key — no
//! scheme can make it one — so this module ships both forms and is explicit
//! about which is which:
//!
//! | Form | Words | Bits | Invertible? |
//! |------|-------|------|-------------|
//! | [`KeyNameNaming::short_name`] | 8 | 88 (BLAKE3 fingerprint) | No — resolves only via the local directory |
//! | [`KeyNameNaming::full_name`] | 24 | 256 key + 8 checksum = 264 | **Yes** — pure encoding, exact round-trip |
//!
//! The short form is the human-facing display name and behaves like a
//! fingerprint: you learn a key (from a signed ad, a contact exchange, a
//! message), the node remembers it under its 8-word name, and that name then
//! resolves. The 24-word form is a complete transcribable key. 88 bits of
//! fingerprint puts the birthday bound at ~2⁴⁴ keys, and a *targeted* forgery
//! costs ~2⁸⁸ work; the substrate never trusts a name either way.
//!
//! # The rule that keeps this safe
//!
//! Names are a **display layer over raw keys**. Every authorization decision in
//! the tree is made against a [`PubKey`], never against a name. Resolution
//! failures return `None` — this provider never guesses, never fuzzy-matches,
//! and never panics on malformed input.

pub mod wordlist;

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{Result, SeamError};
use crate::identity::PubKey;
use crate::naming::Naming;
use wordlist::WORDLIST;

/// Bits of entropy carried by one word (`2^11 == 2048`).
pub const BITS_PER_WORD: usize = 11;
/// Words in the human-facing short (fingerprint) key-name.
pub const SHORT_WORDS: usize = 8;
/// Words in the lossless, fully invertible key-name (256-bit key + 8-bit checksum).
pub const FULL_WORDS: usize = 24;

/// Domain-separation tag so a key-name fingerprint can never collide with any
/// other BLAKE3 use of the same key elsewhere in the tree.
const FINGERPRINT_CONTEXT: &[u8] = b"magnetite-seams/keyname/v1/short";

/// Pack a byte slice into 11-bit groups (MSB-first) and render as words.
fn words_from_bytes(bytes: &[u8], words: usize) -> String {
    let mut out: Vec<&str> = Vec::with_capacity(words);
    let mut acc: u32 = 0;
    let mut acc_bits: usize = 0;
    let mut iter = bytes.iter();
    for _ in 0..words {
        while acc_bits < BITS_PER_WORD {
            let b = *iter.next().unwrap_or(&0);
            acc = (acc << 8) | b as u32;
            acc_bits += 8;
        }
        let shift = acc_bits - BITS_PER_WORD;
        let idx = ((acc >> shift) & 0x7FF) as usize;
        acc &= (1u32 << shift) - 1;
        acc_bits = shift;
        out.push(WORDLIST[idx]);
    }
    out.join("-")
}

/// Look a word up in the list. `None` for anything not in the list (typos,
/// junk, wrong-language words) — callers turn that into a clean failure.
fn word_index(w: &str) -> Option<u16> {
    WORDLIST.binary_search(&w).ok().map(|i| i as u16)
}

/// Split a name into normalized word tokens. Accepts `-`, `_`, `.` and
/// whitespace as separators and is case-insensitive.
fn tokenize(name: &str) -> Vec<String> {
    name.split(|c: char| c == '-' || c == '_' || c == '.' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

/// The checksum byte appended to a key before 24-word encoding.
fn checksum_byte(pk: &PubKey) -> u8 {
    blake3::hash(&pk.0).as_bytes()[0]
}

/// Render `pk` as the lossless 24-word name (256 key bits + 8 checksum bits).
pub fn full_name_of(pk: &PubKey) -> String {
    let mut buf = [0u8; 33];
    buf[..32].copy_from_slice(&pk.0);
    buf[32] = checksum_byte(pk);
    words_from_bytes(&buf, FULL_WORDS)
}

/// Render `pk` as the 8-word fingerprint name (88 bits of BLAKE3, not invertible).
pub fn short_name_of(pk: &PubKey) -> String {
    let mut h = blake3::Hasher::new();
    h.update(FINGERPRINT_CONTEXT);
    h.update(&pk.0);
    let digest = h.finalize();
    // 8 words × 11 bits = 88 bits = 11 bytes.
    words_from_bytes(&digest.as_bytes()[..11], SHORT_WORDS)
}

/// Parse a lossless 24-word name back into a key. The inverse of
/// [`full_name_of`] — exact for every possible key.
///
/// Fails cleanly (never panics) on the wrong word count, an unknown word, a
/// bad checksum, or trailing bit garbage.
pub fn parse_full_name(name: &str) -> Result<PubKey> {
    let toks = tokenize(name);
    if toks.len() != FULL_WORDS {
        return Err(SeamError::MalformedKey(format!(
            "key-name must be {FULL_WORDS} words, got {}",
            toks.len()
        )));
    }
    let mut buf = [0u8; 33];
    let mut acc: u32 = 0;
    let mut acc_bits: usize = 0;
    let mut out = 0usize;
    for t in &toks {
        let idx = word_index(t)
            .ok_or_else(|| SeamError::MalformedKey(format!("unknown key-name word: {t:?}")))?;
        acc = (acc << BITS_PER_WORD) | idx as u32;
        acc_bits += BITS_PER_WORD;
        while acc_bits >= 8 {
            let shift = acc_bits - 8;
            buf[out] = ((acc >> shift) & 0xFF) as u8;
            out += 1;
            acc &= (1u32 << shift) - 1;
            acc_bits = shift;
        }
    }
    debug_assert_eq!(out, 33);
    debug_assert_eq!(acc_bits, 0);
    let mut key = [0u8; 32];
    key.copy_from_slice(&buf[..32]);
    let pk = PubKey(key);
    if buf[32] != checksum_byte(&pk) {
        return Err(SeamError::MalformedKey(
            "key-name checksum mismatch (mistyped word?)".into(),
        ));
    }
    Ok(pk)
}

/// Word-based [`Naming`] provider — the optional, feature-gated alternative to
/// [`HashNaming`](crate::naming::HashNaming). See the module docs.
///
/// Resolution order (all fail-closed, `None` on anything unrecognized):
/// 1. a 24-word lossless name → decoded directly, no directory needed;
/// 2. an 8-word name, optionally `…@domain` → looked up in the local directory
///    of keys this node has *learned*, and if the name carries a domain it must
///    match the learned hint;
/// 3. a raw 64-char hex key, if [`accept_hex`](Self::accept_hex) (default `true`) —
///    the substrate form is always addressable.
pub struct KeyNameNaming {
    /// 8-word short name → key, for keys this node has seen.
    directory: Mutex<HashMap<String, PubKey>>,
    /// Optional per-key domain hint, for `<words>@domain` display.
    domains: Mutex<HashMap<PubKey, String>>,
    /// Whether raw hex addresses still resolve. Substrate access is on by default.
    pub accept_hex: bool,
}

impl Default for KeyNameNaming {
    fn default() -> Self {
        Self {
            directory: Mutex::new(HashMap::new()),
            domains: Mutex::new(HashMap::new()),
            accept_hex: true,
        }
    }
}

impl KeyNameNaming {
    /// Empty provider with no learned keys.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a key so its 8-word short name becomes resolvable. Idempotent.
    ///
    /// This is a *hint cache*, not an authority: learning a key grants it
    /// nothing, and the short name is only ever a display for the key bytes.
    pub fn learn(&self, pk: PubKey) {
        self.directory
            .lock()
            .unwrap()
            .insert(short_name_of(&pk), pk);
    }

    /// Learn a key and attach a domain hint, enabling `<words>@domain` display
    /// and requiring that domain when a caller resolves the qualified form.
    pub fn learn_with_domain(&self, pk: PubKey, domain: &str) {
        self.learn(pk);
        self.domains
            .lock()
            .unwrap()
            .insert(pk, domain.to_ascii_lowercase());
    }

    /// The 8-word fingerprint name (the display form).
    pub fn short_name(&self, pk: &PubKey) -> String {
        short_name_of(pk)
    }

    /// The lossless 24-word name (a fully transcribable key).
    pub fn full_name(&self, pk: &PubKey) -> String {
        full_name_of(pk)
    }

    /// The canonical, name-less address for a key (full hex) — same substrate
    /// form [`HashNaming`](crate::naming::HashNaming) uses.
    pub fn canonical(&self, pk: &PubKey) -> String {
        pk.to_hex()
    }
}

#[async_trait::async_trait]
impl Naming for KeyNameNaming {
    async fn resolve(&self, name: &str) -> Option<PubKey> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Split an optional `@domain` suffix. More than one '@' is malformed.
        let mut parts = trimmed.split('@');
        let local = parts.next()?;
        let domain = parts.next().map(|d| d.to_ascii_lowercase());
        if parts.next().is_some() {
            return None;
        }
        if domain.as_deref().is_some_and(str::is_empty) {
            return None;
        }

        let toks = tokenize(local);
        match toks.len() {
            FULL_WORDS => {
                // Lossless form: decode outright. A domain suffix here must
                // still agree with any learned hint.
                let pk = parse_full_name(local).ok()?;
                self.domain_ok(&pk, domain.as_deref()).then_some(pk)
            }
            SHORT_WORDS => {
                let key = toks.join("-");
                let pk = *self.directory.lock().unwrap().get(&key)?;
                self.domain_ok(&pk, domain.as_deref()).then_some(pk)
            }
            1 if self.accept_hex && domain.is_none() => PubKey::from_hex(&toks[0]).ok(),
            _ => None,
        }
    }

    fn display(&self, pk: &PubKey) -> String {
        let short = short_name_of(pk);
        match self.domains.lock().unwrap().get(pk) {
            Some(d) => format!("{short}@{d}"),
            None => short,
        }
    }
}

impl KeyNameNaming {
    /// A qualified name must match the key's learned domain hint; an
    /// unqualified name always passes. Fail-closed: a name asserting a domain
    /// we have no hint for, or a different one, does not resolve.
    fn domain_ok(&self, pk: &PubKey, domain: Option<&str>) -> bool {
        match domain {
            None => true,
            Some(d) => self.domains.lock().unwrap().get(pk).map(String::as_str) == Some(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use std::collections::HashSet;

    fn random_key() -> PubKey {
        let mut b = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut b);
        PubKey(b)
    }

    // ---- wordlist invariants (a corrupted list must fail CI, not rename keys) ----

    #[test]
    fn wordlist_is_2048_unique_sorted_ascii_words() {
        assert_eq!(WORDLIST.len(), 2048, "11 bits per word requires exactly 2048");
        let set: HashSet<&&str> = WORDLIST.iter().collect();
        assert_eq!(set.len(), 2048, "duplicate words would make names ambiguous");
        let mut sorted = WORDLIST;
        sorted.sort_unstable();
        assert_eq!(sorted, WORDLIST, "binary_search lookup requires sorted order");
        for w in WORDLIST {
            assert!(
                (4..=7).contains(&w.len()) && w.chars().all(|c| c.is_ascii_lowercase()),
                "bad word: {w:?}"
            );
        }
    }

    #[test]
    fn wordlist_prefixes_are_unique() {
        let prefixes: HashSet<&str> = WORDLIST.iter().map(|w| &w[..4]).collect();
        assert_eq!(prefixes.len(), 2048, "every word must have a unique 4-char prefix");
    }

    // ---- lossless 24-word form: exact round trip ----

    #[test]
    fn full_name_roundtrips_for_random_keys() {
        for _ in 0..512 {
            let pk = random_key();
            let name = full_name_of(&pk);
            assert_eq!(name.split('-').count(), FULL_WORDS);
            assert_eq!(parse_full_name(&name).expect("decodes"), pk, "round trip");
        }
    }

    #[test]
    fn full_name_roundtrips_for_edge_keys() {
        for pk in [PubKey([0u8; 32]), PubKey([0xFFu8; 32])] {
            assert_eq!(parse_full_name(&full_name_of(&pk)).unwrap(), pk);
        }
    }

    #[test]
    fn full_names_are_distinct_for_distinct_keys() {
        let mut seen = HashSet::new();
        for i in 0..=255u8 {
            assert!(seen.insert(full_name_of(&PubKey([i; 32]))), "collision at {i}");
        }
    }

    #[test]
    fn full_name_parsing_is_case_and_separator_insensitive() {
        let pk = random_key();
        let name = full_name_of(&pk);
        assert_eq!(parse_full_name(&name.to_uppercase()).unwrap(), pk);
        assert_eq!(parse_full_name(&name.replace('-', " ")).unwrap(), pk);
        assert_eq!(parse_full_name(&name.replace('-', ".")).unwrap(), pk);
    }

    // ---- malformed input fails cleanly, never panics ----

    #[test]
    fn malformed_full_names_fail_cleanly() {
        let pk = random_key();
        let name = full_name_of(&pk);
        let mut words: Vec<&str> = name.split('-').collect();

        // Wrong length.
        assert!(parse_full_name("").is_err());
        assert!(parse_full_name(&words[..23].join("-")).is_err());
        assert!(parse_full_name(&format!("{name}-{}", words[0])).is_err());
        // Unknown / misspelled word.
        words[3] = "zzzznotaword";
        assert!(parse_full_name(&words.join("-")).is_err());
        // A single swapped-but-valid word breaks the checksum with p ≈ 255/256.
        let mut broken = 0;
        for w in WORDLIST.iter().take(64) {
            let mut ws: Vec<&str> = name.split('-').collect();
            if ws[0] == *w {
                continue;
            }
            ws[0] = w;
            if parse_full_name(&ws.join("-")).is_err() {
                broken += 1;
            }
        }
        assert!(broken >= 60, "checksum should catch nearly every single-word typo");
    }

    #[test]
    fn arbitrary_junk_never_panics() {
        let naming = KeyNameNaming::new();
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        for junk in [
            "",
            "   ",
            "@",
            "@domain",
            "a@b@c",
            "-----",
            "...",
            "abet",
            "abet-abet",
            "not a key name at all",
            "\u{0}\u{1}\u{7f}",
            "𝔞𝔟𝔠",
            "ff",
            &"abet-".repeat(200),
        ] {
            assert_eq!(rt.block_on(naming.resolve(junk)), None, "junk: {junk:?}");
            assert!(parse_full_name(junk).is_err());
        }
    }

    // ---- short (fingerprint) form + directory ----

    #[test]
    fn short_names_are_eight_words_deterministic_and_distinct() {
        let a = PubKey([1u8; 32]);
        let b = PubKey([2u8; 32]);
        assert_eq!(short_name_of(&a).split('-').count(), SHORT_WORDS);
        assert_eq!(short_name_of(&a), short_name_of(&a));
        assert_ne!(short_name_of(&a), short_name_of(&b));

        let mut seen = HashSet::new();
        for _ in 0..2000 {
            assert!(seen.insert(short_name_of(&random_key())), "88-bit collision");
        }
    }

    #[tokio::test]
    async fn learned_short_name_resolves_and_unlearned_does_not() {
        let naming = KeyNameNaming::new();
        let pk = random_key();
        let name = short_name_of(&pk);
        assert_eq!(naming.resolve(&name).await, None, "unknown key must not resolve");
        naming.learn(pk);
        assert_eq!(naming.resolve(&name).await, Some(pk));
        assert_eq!(naming.display(&pk), name);
    }

    #[tokio::test]
    async fn lossless_name_resolves_without_being_learned() {
        let naming = KeyNameNaming::new();
        let pk = random_key();
        assert_eq!(naming.resolve(&full_name_of(&pk)).await, Some(pk));
    }

    #[tokio::test]
    async fn domain_hint_displays_and_must_match_to_resolve() {
        let naming = KeyNameNaming::new();
        let pk = random_key();
        naming.learn_with_domain(pk, "Example.ORG");
        let short = short_name_of(&pk);
        assert_eq!(naming.display(&pk), format!("{short}@example.org"));
        assert_eq!(naming.resolve(&format!("{short}@example.org")).await, Some(pk));
        // Unqualified still works…
        assert_eq!(naming.resolve(&short).await, Some(pk));
        // …but a WRONG domain fails closed rather than falling back to the key.
        assert_eq!(naming.resolve(&format!("{short}@evil.test")).await, None);

        // A key with no hint rejects any qualified form.
        let other = random_key();
        naming.learn(other);
        let os = short_name_of(&other);
        assert_eq!(naming.resolve(&os).await, Some(other));
        assert_eq!(naming.resolve(&format!("{os}@example.org")).await, None);
    }

    #[tokio::test]
    async fn raw_hex_substrate_address_still_resolves_and_can_be_disabled() {
        let mut naming = KeyNameNaming::new();
        let pk = random_key();
        assert_eq!(naming.resolve(&pk.to_hex()).await, Some(pk));
        naming.accept_hex = false;
        assert_eq!(naming.resolve(&pk.to_hex()).await, None);
    }
}
