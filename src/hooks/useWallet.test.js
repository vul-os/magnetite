// useWallet.test.js — tests for the USD fiat contract (D-PAY-1).
// Platform is now fiat-only (USD); payouts go via Wise, deposits via Paystack.
// walletAddress (crypto on-chain) is removed. withdraw = payout request.
// Mock data is only used when VITE_USE_MOCKS=true (default off).
// These tests assert the real-fetch path by mocking the api client.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWallet } from './useWallet';

vi.mock('../api/client', () => ({
  api: {
    wallet: {
      balance: vi.fn(),
      transactions: vi.fn(),
      deposit: vi.fn(),
      withdraw: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

describe('useWallet', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: API fails → hook enters error state (not silent mock success).
    api.wallet.balance.mockRejectedValue(new Error('No backend'));
    api.wallet.transactions.mockRejectedValue(new Error('No backend'));
    api.wallet.deposit.mockRejectedValue(new Error('No backend'));
    api.wallet.withdraw.mockRejectedValue(new Error('No backend'));
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

  it('sets error when balance API call fails', async () => {
    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeTruthy();
  });

  it('populates USD balance from API response', async () => {
    api.wallet.balance.mockResolvedValue({ data: { balance: '250.75' } });
    api.wallet.transactions.mockRejectedValue(new Error('no tx'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.balance).toBe(250.75);
    // No walletAddress — crypto on-chain address is removed (D-PAY-1).
    expect(result.current.walletAddress).toBeUndefined();
    expect(result.current.error).toBeNull();
  });

  it('populates balance from flat API response (no data wrapper)', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '99.50' });
    api.wallet.transactions.mockRejectedValue(new Error('no tx'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.balance).toBe(99.5);
  });

  it('populates transactions from API response', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '100' });
    const fakeTxns = [
      { id: 1, type: 'deposit', amount: 100, status: 'completed' },
      { id: 2, type: 'withdraw', amount: -50, status: 'completed' },
    ];
    api.wallet.transactions.mockResolvedValue({ transactions: fakeTxns });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.transactions).toEqual(fakeTxns);
  });

  it('uses empty array when transactions API fails (non-fatal)', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '500' });
    api.wallet.transactions.mockRejectedValue(new Error('tx fail'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    // Balance loaded OK, transactions empty due to failure.
    expect(result.current.balance).toBe(500);
    expect(result.current.transactions).toEqual([]);
  });

  // ── deposit (Paystack fiat on-ramp) ──────────────────────────────────────

  it('deposit: calls real API with amount and method', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '200' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.deposit.mockResolvedValue({ balance: '300' });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deposit(100, 'paystack');
    });

    expect(api.wallet.deposit).toHaveBeenCalledWith({ amount: 100, method: 'paystack' });
  });

  it('deposit: updates USD balance from API response', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '200' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.deposit.mockResolvedValue({ balance: '350' });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deposit(150, 'paystack');
    });

    expect(result.current.balance).toBe(350);
  });

  it('deposit: falls back to optimistic update when API does not return balance', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '100' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    // API responds OK but no balance field.
    api.wallet.deposit.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deposit(50, 'paystack');
    });

    // Optimistic: balance += amount
    expect(result.current.balance).toBe(150);
  });

  it('deposit: propagates error when API fails', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '100' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.deposit.mockRejectedValue(new Error('Payment failed'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.deposit(50, 'paystack');
      } catch (e) {
        caughtError = e;
      }
    });

    // Error propagates (no silent success).
    expect(caughtError).toBeDefined();
    expect(caughtError.message).toBe('Payment failed');
  });

  it('deposit: adds a transaction to the list on success', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '100' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.deposit.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const txBefore = result.current.transactions.length;

    await act(async () => {
      await result.current.deposit(200, 'paystack');
    });

    expect(result.current.transactions.length).toBe(txBefore + 1);
    expect(result.current.transactions[0].type).toBe('deposit');
  });

  // ── withdraw = payout request (Wise, D-PAY-4) ────────────────────────────

  it('withdraw: calls real API with amount (payout request)', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '500' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.withdraw.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.withdraw(100);
    });

    expect(api.wallet.withdraw).toHaveBeenCalledWith({ amount: 100 });
  });

  it('withdraw: throws Insufficient balance error when balance too low', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '50' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.withdraw(200);
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
    expect(caughtError.message).toMatch(/[Ii]nsufficient/);
    expect(api.wallet.withdraw).not.toHaveBeenCalled();
  });

  it('withdraw: propagates error when API fails', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '500' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.withdraw.mockRejectedValue(new Error('Withdrawal blocked'));

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let caughtError;
    await act(async () => {
      try {
        await result.current.withdraw(100);
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
  });

  it('withdraw: adds a payout transaction to the list on success', async () => {
    api.wallet.balance.mockResolvedValue({ balance: '500' });
    api.wallet.transactions.mockResolvedValue({ transactions: [] });
    api.wallet.withdraw.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const txBefore = result.current.transactions.length;

    await act(async () => {
      await result.current.withdraw(100);
    });

    expect(result.current.transactions.length).toBe(txBefore + 1);
    expect(result.current.transactions[0].type).toBe('withdraw');
  });

  // ── Return shape ──────────────────────────────────────────────────────────

  it('exposes the expected shape from the hook (USD, no walletAddress)', async () => {
    const { result } = renderHook(() => useWallet());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(typeof result.current.deposit).toBe('function');
    expect(typeof result.current.withdraw).toBe('function');
    expect(Array.isArray(result.current.transactions)).toBe(true);
    // walletAddress removed — crypto on-chain address is no longer exposed.
    expect(result.current.walletAddress).toBeUndefined();
  });
});
