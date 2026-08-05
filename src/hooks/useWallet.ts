import { useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';

/**
 * useWallet — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
 *
 * This node never holds funds, so there is nothing here to "top up" or "cash
 * out". The wallet is an *address*; the ledger is a list of *signed receipts*.
 *
 *   - `address`     — the linked hex Ed25519 pubkey, or null if unlinked.
 *   - `custodial`   — always false. Rendered so the claim is visible, not implied.
 *   - `rail`        — which PaymentRail settled the receipts (`mock` offline).
 *   - `receipts`    — replaces the old custodial transaction ledger.
 *   - `link()`      — point the account at a wallet you control.
 *
 * `deposit()` / `withdraw()` are deliberately absent: the backend routes
 * (`POST /wallet/deposit`, `POST /wallet/withdraw`) and the `wallet_balances` /
 * `wallet_transactions` tables were deleted.
 */

export interface Receipt {
  id: string;
  kind: string;
  subject: string;
  total: number;
  protocol_fee: number;
  counterparty: string;
  rail_pubkey: string;
  voided: boolean;
  created_at: string;
}

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Mock data — only used when VITE_USE_MOCKS === 'true'. Deterministic so the
// screenshotter produces stable images with no backend and no database.
const MOCK_ADDRESS =
  '9f2c41a7be03d85610fa27cc4e91b8d3705ea6c2149fbb8e37d0c5a94162e7b0';

const MOCK_RECEIPTS: Receipt[] = [
  {
    id: 'rcpt_01HQ8ZK3NP',
    kind: 'item_purchase',
    subject: 'Plasma Rifle Skin · Cosmic Raiders',
    total: 99,
    protocol_fee: 0,
    counterparty: 'd41b9c07a5e8f236104b7dd9ce8215af6390b7e04c2d18f5a6b93e70d1c4825f',
    rail_pubkey: '3ac9017e5fb2d846902ce15b7a4d3f80c6e1927b5d0af348e2c76b91045fd8a2',
    voided: false,
    created_at: '2026-07-16T18:41:00Z',
  },
  {
    id: 'rcpt_01HQ7WX8TM',
    kind: 'hosting_fee',
    subject: 'Seat on nord-fjord-01 · 2h',
    total: 40,
    protocol_fee: 0,
    counterparty: '7e30b8a1cd94c25e06f381ba47d9e2c0518736fa9db4e18c02735d6ab91af4e2',
    rail_pubkey: '3ac9017e5fb2d846902ce15b7a4d3f80c6e1927b5d0af348e2c76b91045fd8a2',
    voided: false,
    created_at: '2026-07-15T09:12:00Z',
  },
  {
    id: 'rcpt_01HQ5MC2QD',
    kind: 'wager',
    subject: 'Ranked match wager · Cosmic Raiders',
    total: 500,
    protocol_fee: 0,
    counterparty: 'b0561ea38c7d29f4013a8ce65b27d09f4e81a6c37d520fb98e14c7302a6db85f',
    rail_pubkey: '3ac9017e5fb2d846902ce15b7a4d3f80c6e1927b5d0af348e2c76b91045fd8a2',
    voided: false,
    created_at: '2026-07-01T07:03:00Z',
  },
  {
    id: 'rcpt_01HQ2FA9RB',
    kind: 'item_purchase',
    subject: 'Carbon Livery · Speed Legends',
    total: 149,
    protocol_fee: 0,
    counterparty: '5c8de401f9b7236a0d14e8c93b750af26e1d3809c47ba62e91d70b385ac64f13',
    rail_pubkey: '3ac9017e5fb2d846902ce15b7a4d3f80c6e1927b5d0af348e2c76b91045fd8a2',
    voided: true,
    created_at: '2026-06-24T20:55:00Z',
  },
];

/** Coerce whatever envelope the backend used into a plain array. */
function asList(payload: unknown): Receipt[] {
  const body = (payload as { data?: unknown } | null)?.data ?? payload;
  // Array.isArray's built-in signature is `(arg: any): arg is any[]`, so it
  // narrows to any[] regardless of the checked variable's prior type — these
  // casts make the trust this function already documents (coercing an
  // unvalidated backend envelope into the expected shape) explicit instead
  // of an implicit any[] return.
  if (Array.isArray(body)) return body as Receipt[];
  const receipts = (body as { receipts?: unknown } | null)?.receipts;
  if (Array.isArray(receipts)) return receipts as Receipt[];
  const items = (body as { items?: unknown } | null)?.items;
  if (Array.isArray(items)) return items as Receipt[];
  return [];
}

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

export function useWallet() {
  const [address, setAddress] = useState<string | null>(USE_MOCKS ? MOCK_ADDRESS : null);
  const [rail, setRail] = useState<string | null>(USE_MOCKS ? 'mock' : null);
  const [receipts, setReceipts] = useState<Receipt[]>(USE_MOCKS ? MOCK_RECEIPTS : []);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadWallet() {
      setLoading(true);
      setError(null);
      try {
        const [walletResult, receiptResult] = await Promise.allSettled([
          api.wallet.get(),
          api.wallet.receipts(),
        ]);

        if (cancelled) return;

        if (walletResult.status === 'fulfilled') {
          const payload = (walletResult.value as { data?: { wallet_address?: string; rail?: string } } | null)?.data
            ?? (walletResult.value as { wallet_address?: string; rail?: string } | null);
          setAddress(payload?.wallet_address ?? null);
          setRail(payload?.rail ?? null);
        } else {
          throw walletResult.reason;
        }

        // A failed receipt fetch is non-fatal; the address still renders.
        if (receiptResult.status === 'fulfilled') {
          setReceipts(asList(receiptResult.value));
        }
      } catch (err) {
        if (!cancelled) setError(errMessage(err, 'Failed to load wallet'));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void loadWallet();
    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Link (or replace) the wallet this account is paid to / charged from.
   * Errors propagate so the caller can surface them.
   */
  const link = useCallback(async (walletAddress: string): Promise<string> => {
    const clean = String(walletAddress || '').trim().replace(/^0x/, '');
    if (!/^[0-9a-fA-F]{64}$/.test(clean)) {
      throw new Error('Wallet address must be a 32-byte hex Ed25519 public key');
    }

    if (USE_MOCKS) {
      setAddress(clean.toLowerCase());
      return clean.toLowerCase();
    }

    const result = await api.wallet.link(clean);
    const payload = (result as { data?: { wallet_address?: string; rail?: string } } | null)?.data
      ?? (result as { wallet_address?: string; rail?: string } | null);
    const next = payload?.wallet_address ?? clean.toLowerCase();
    setAddress(next);
    if (payload?.rail) setRail(payload.rail);
    return next;
  }, []);

  return {
    address,
    /** Always false — this node never takes custody of user funds. */
    custodial: false,
    rail,
    receipts,
    link,
    loading,
    error,
  };
}
