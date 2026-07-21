//! Wire-format helpers for Solana: base58 pubkeys, program ids, associated
//! token account (PDA) derivation, SPL `TransferChecked` + Memo instruction
//! encoding, and legacy-message serialization.
//!
//! This is deliberately a *small*, self-contained implementation rather than a
//! dependency on `solana-sdk`: the seam crate must stay light and must build
//! offline. Everything here is pure byte layout — no network, no floats.

use crate::identity::PubKey;

use super::SolanaError;

/// `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
/// SPL Memo v3 — `MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr`
pub const MEMO_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

/// Canonical USDC mint on Solana mainnet-beta.
pub const USDC_MAINNET_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
/// USDC on devnet (Circle's devnet mint).
pub const USDC_DEVNET_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

/// USDC has **6** decimals. All amounts in this module are integer smallest
/// units ("micro-USDC"); there are no floats anywhere in the money path.
pub const USDC_DECIMALS: u8 = 6;

/// Decode a base58 Solana address into raw 32 bytes.
pub fn pubkey_from_base58(s: &str) -> Result<PubKey, SolanaError> {
    let raw = bs58::decode(s)
        .into_vec()
        .map_err(|_| SolanaError::BadAddress(s.to_string()))?;
    let arr: [u8; 32] = raw
        .try_into()
        .map_err(|_| SolanaError::BadAddress(s.to_string()))?;
    Ok(PubKey(arr))
}

/// Encode raw 32 bytes as a base58 Solana address.
pub fn pubkey_to_base58(k: &PubKey) -> String {
    bs58::encode(k.0).into_string()
}

/// Is a compressed Edwards point actually on the ed25519 curve? A program
/// derived address must NOT be (that is what makes it unsignable).
fn is_on_curve(bytes: &[u8; 32]) -> bool {
    curve25519_dalek::edwards::CompressedEdwardsY(*bytes)
        .decompress()
        .is_some()
}

/// `find_program_address` — hash the seeds with a descending bump until the
/// result falls off the curve.
fn find_program_address(seeds: &[&[u8]], program_id: &PubKey) -> Result<(PubKey, u8), SolanaError> {
    use sha2::{Digest, Sha256};
    for bump in (0u8..=255).rev() {
        let mut h = Sha256::new();
        for s in seeds {
            h.update(s);
        }
        h.update([bump]);
        h.update(program_id.0);
        h.update(b"ProgramDerivedAddress");
        let out: [u8; 32] = h.finalize().into();
        if !is_on_curve(&out) {
            return Ok((PubKey(out), bump));
        }
    }
    Err(SolanaError::Derivation)
}

/// The associated token account that holds `mint` for `owner`.
pub fn associated_token_address(owner: &PubKey, mint: &PubKey) -> Result<PubKey, SolanaError> {
    let token = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;
    let (addr, _bump) =
        find_program_address(&[&owner.0, &token.0, &mint.0], &ata_program)?;
    Ok(addr)
}

// ── Instruction / message encoding ───────────────────────────────────────────

/// One instruction, in the pre-compilation form (real keys, not indices).
#[derive(Clone, Debug)]
pub struct Instruction {
    /// Program to invoke.
    pub program_id: PubKey,
    /// `(pubkey, is_signer, is_writable)` for each account.
    pub accounts: Vec<(PubKey, bool, bool)>,
    /// Opaque instruction data.
    pub data: Vec<u8>,
}

/// SPL Token `TransferChecked` (discriminant 12). Encoding the decimals into
/// the instruction is what makes the transfer *checked*: the runtime rejects it
/// if the mint's decimals differ, so a 6-decimal USDC amount can never be
/// silently reinterpreted against an 9-decimal mint.
pub fn transfer_checked(
    source_ata: PubKey,
    mint: PubKey,
    dest_ata: PubKey,
    authority: PubKey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let mut data = Vec::with_capacity(10);
    data.push(12u8);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);
    Instruction {
        program_id: pubkey_from_base58(TOKEN_PROGRAM_ID).expect("const token program id"),
        accounts: vec![
            (source_ata, false, true),
            (mint, false, false),
            (dest_ata, false, true),
            (authority, true, false),
        ],
        data,
    }
}

/// SPL Memo — carries the `(buyer, item)` binding on-chain.
pub fn memo(signer: PubKey, note: &str) -> Instruction {
    Instruction {
        program_id: pubkey_from_base58(MEMO_PROGRAM_ID).expect("const memo program id"),
        accounts: vec![(signer, true, false)],
        data: note.as_bytes().to_vec(),
    }
}

fn compact_u16(v: usize, out: &mut Vec<u8>) {
    let mut rem = v;
    loop {
        let mut byte = (rem & 0x7f) as u8;
        rem >>= 7;
        if rem == 0 {
            out.push(byte);
            break;
        }
        byte |= 0x80;
        out.push(byte);
    }
}

/// Serialize a legacy (v0-less) transaction message.
///
/// `payer` is account index 0 and the sole required signature. Account ordering
/// follows the Solana rule: writable-signers, readonly-signers, writable
/// non-signers, readonly non-signers.
pub fn serialize_message(
    payer: &PubKey,
    instructions: &[Instruction],
    recent_blockhash: &[u8; 32],
) -> Vec<u8> {
    // Collect accounts with merged flags.
    let mut keys: Vec<(PubKey, bool, bool)> = vec![(*payer, true, true)];
    let add = |k: PubKey, signer: bool, writable: bool, keys: &mut Vec<(PubKey, bool, bool)>| {
        if let Some(e) = keys.iter_mut().find(|e| e.0 == k) {
            e.1 |= signer;
            e.2 |= writable;
        } else {
            keys.push((k, signer, writable));
        }
    };
    for ix in instructions {
        for (k, s, w) in &ix.accounts {
            add(*k, *s, *w, &mut keys);
        }
    }
    // Program ids are readonly non-signers.
    for ix in instructions {
        add(ix.program_id, false, false, &mut keys);
    }

    let rank = |e: &(PubKey, bool, bool)| match (e.1, e.2) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    // Stable sort keeps the payer first within rank 0.
    keys.sort_by_key(rank);

    let num_signers = keys.iter().filter(|e| e.1).count() as u8;
    let num_readonly_signed = keys.iter().filter(|e| e.1 && !e.2).count() as u8;
    let num_readonly_unsigned = keys.iter().filter(|e| !e.1 && !e.2).count() as u8;

    let index_of = |k: &PubKey| keys.iter().position(|e| e.0 == *k).unwrap() as u8;

    let mut out = Vec::new();
    out.push(num_signers);
    out.push(num_readonly_signed);
    out.push(num_readonly_unsigned);
    compact_u16(keys.len(), &mut out);
    for (k, _, _) in &keys {
        out.extend_from_slice(&k.0);
    }
    out.extend_from_slice(recent_blockhash);
    compact_u16(instructions.len(), &mut out);
    for ix in instructions {
        out.push(index_of(&ix.program_id));
        compact_u16(ix.accounts.len(), &mut out);
        for (k, _, _) in &ix.accounts {
            out.push(index_of(k));
        }
        compact_u16(ix.data.len(), &mut out);
        out.extend_from_slice(&ix.data);
    }
    out
}

/// Prefix a serialized message with its (single) signature to make a wire
/// transaction.
pub fn wire_transaction(signature: &[u8; 64], message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 64 + message.len());
    compact_u16(1, &mut out);
    out.extend_from_slice(signature);
    out.extend_from_slice(message);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_roundtrips() {
        let k = pubkey_from_base58(USDC_MAINNET_MINT).unwrap();
        assert_eq!(pubkey_to_base58(&k), USDC_MAINNET_MINT);
    }

    #[test]
    fn rejects_bad_address() {
        assert!(pubkey_from_base58("not-base58-0OIl").is_err());
        assert!(pubkey_from_base58("abc").is_err(), "wrong length");
    }

    #[test]
    fn transfer_checked_encodes_amount_and_decimals() {
        let z = PubKey([0u8; 32]);
        let ix = transfer_checked(z, z, z, z, 1_500_000, USDC_DECIMALS);
        assert_eq!(ix.data[0], 12);
        assert_eq!(&ix.data[1..9], &1_500_000u64.to_le_bytes());
        assert_eq!(ix.data[9], 6);
    }

    #[test]
    fn ata_is_off_curve_and_deterministic() {
        let owner = PubKey([3u8; 32]);
        let mint = pubkey_from_base58(USDC_MAINNET_MINT).unwrap();
        let a = associated_token_address(&owner, &mint).unwrap();
        let b = associated_token_address(&owner, &mint).unwrap();
        assert_eq!(a, b);
        assert!(!is_on_curve(&a.0), "a PDA must not be a signable key");
        let other = associated_token_address(&PubKey([4u8; 32]), &mint).unwrap();
        assert_ne!(a, other);
    }

    #[test]
    fn message_places_payer_first_and_is_deterministic() {
        let payer = PubKey([1u8; 32]);
        let mint = pubkey_from_base58(USDC_MAINNET_MINT).unwrap();
        let ix = transfer_checked(PubKey([9; 32]), mint, PubKey([8; 32]), payer, 10, 6);
        let m1 = serialize_message(&payer, &[ix.clone()], &[7u8; 32]);
        let m2 = serialize_message(&payer, &[ix], &[7u8; 32]);
        assert_eq!(m1, m2);
        assert_eq!(m1[0], 1, "exactly one signer");
        assert_eq!(&m1[4..36], &payer.0, "payer is account index 0");
    }

    #[test]
    fn compact_u16_multibyte() {
        let mut v = Vec::new();
        compact_u16(0x81, &mut v);
        assert_eq!(v, vec![0x81, 0x01]);
    }
}
