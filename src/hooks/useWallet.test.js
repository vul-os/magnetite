// useWallet.test.js — NON-CUSTODIAL wallet contract (seam §3.6 `PaymentRail`).
//
// There are no balances, deposits, withdrawals or payouts: the node never takes
// custody. The hook exposes an *address* the user controls plus the list of
// *signed receipts* that replaced the custodial transaction ledger.
// Mock data is only used when VITE_USE_MOCKS=true (default off), so these tests
// exercise the real-fetch path by mocking the api client.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWallet } from './useWallet';

vi.mock('../api/client', () => ({
  api: {
    wallet: {
      get: vi.fn(),
      link: vi.fn(),
      receipts: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

const ADDRESS =
  '9f2c41a7be03d85610fa27cc4e91b8d3705ea6c2149fbb8e37d0c5a94162e7b0';
const OTHER_ADDRESS =
  '1122334455667788990011223344556677889900112233445566778899001122';

const RECEIPTS = [
  {
    id: 'rcpt_01',
    kind: 'item_purchase',
    total: 99,
    protocol_fee: 0,
    rail_pubkey: 'aa'.repeat(32),
    voided: false,
    created_at: '2026-07-16T18:41:00Z',
  },
  {
    id: 'rcpt_02',
    kind: 'hosting_fee',
    total: 40,
    protocol_fee: 0,
    rail_pubkey: 'aa'.repeat(32),
    voided: false,
    created_at: '2026-07-15T09:12:00Z',
  },
];

describe('useWallet', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: API fails → hook enters error state (not silent mock success).
    api.wallet.get.mockRejectedValue(new Error('No backend'));
    api.wallet.receipts.mockRejectedValue(new Error('No backend'));
    api.wallet.link.mockRejectedValue(new Error('No backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── Loading / data states ─────────────────────────────────────────────────

  it('starts in loading state when VITE_USE_MOCKS is off', async () => {
    const { result } = renderHook(() => useWallet());
    // Initially loading (real-fetch path, not instantly populated from mock constants).
    expect(result.current.loading).toBe(true);
    await vi.waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('sets error when the wallet API call fails', async () => {
    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeTruthy();
    expect(result.current.address).toBeNull();
  });

  it('populates address and rail from the API response', async () => {
    api.wallet.get.mockResolvedValue({
      data: { user_id: 'u1', wallet_address: ADDRESS, custodial: false, rail: 'mock' },
    });
    api.wallet.receipts.mockResolvedValue({ receipts: [] });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.address).toBe(ADDRESS);
    expect(result.current.rail).toBe('mock');
    expect(result.current.error).toBeNull();
  });

  it('populates address from a flat API response (no data wrapper)', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: ADDRESS, rail: 'usdc-base' });
    api.wallet.receipts.mockResolvedValue([]);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.address).toBe(ADDRESS);
    expect(result.current.rail).toBe('usdc-base');
  });

  it('leaves address null when the account has no wallet linked yet', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.address).toBeNull();
    expect(result.current.error).toBeNull();
  });

  // ── Receipts (replaces the custodial transaction ledger) ─────────────────

  it('populates receipts from the API response', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: ADDRESS, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue({ receipts: RECEIPTS });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.receipts).toEqual(RECEIPTS);
  });

  it('accepts a bare array of receipts', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: ADDRESS, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue(RECEIPTS);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.receipts).toHaveLength(2);
    expect(result.current.receipts[0].kind).toBe('item_purchase');
  });

  it('treats a failed receipt fetch as non-fatal — the address still renders', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: ADDRESS, rail: 'mock' });
    api.wallet.receipts.mockRejectedValue(new Error('receipts down'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.address).toBe(ADDRESS);
    expect(result.current.receipts).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  // ── link() ────────────────────────────────────────────────────────────────

  it('link: rejects a malformed key without calling the API', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.link('not-a-key');
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
    expect(caughtError.message).toMatch(/hex Ed25519/i);
    expect(api.wallet.link).not.toHaveBeenCalled();
    expect(result.current.address).toBeNull();
  });

  it('link: rejects a hex string of the wrong length', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.link('abcdef');
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
    expect(api.wallet.link).not.toHaveBeenCalled();
  });

  it('link: calls the API with the bare hex key and updates the address', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);
    api.wallet.link.mockResolvedValue({ wallet_address: ADDRESS, rail: 'mock' });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.link(ADDRESS);
    });

    expect(api.wallet.link).toHaveBeenCalledWith(ADDRESS);
    expect(result.current.address).toBe(ADDRESS);
  });

  it('link: strips a 0x prefix before calling the API', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);
    api.wallet.link.mockResolvedValue({ wallet_address: OTHER_ADDRESS });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.link(`0x${OTHER_ADDRESS}`);
    });

    expect(api.wallet.link).toHaveBeenCalledWith(OTHER_ADDRESS);
    expect(result.current.address).toBe(OTHER_ADDRESS);
  });

  it('link: propagates an API failure', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: null, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue([]);
    api.wallet.link.mockRejectedValue(new Error('link rejected'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.link(ADDRESS);
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
    expect(caughtError.message).toBe('link rejected');
  });

  // ── Return shape ──────────────────────────────────────────────────────────

  it('is non-custodial and exposes no balance/deposit/withdraw surface', async () => {
    api.wallet.get.mockResolvedValue({ wallet_address: ADDRESS, rail: 'mock' });
    api.wallet.receipts.mockResolvedValue(RECEIPTS);

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.custodial).toBe(false);
    expect(typeof result.current.link).toBe('function');
    expect(Array.isArray(result.current.receipts)).toBe(true);
    // The custodial surface is gone — these routes no longer exist server-side.
    expect(result.current.balance).toBeUndefined();
    expect(result.current.deposit).toBeUndefined();
    expect(result.current.withdraw).toBeUndefined();
    expect(result.current.transactions).toBeUndefined();
  });
});
