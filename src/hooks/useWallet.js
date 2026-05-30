import { useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';

// Mock data — only used when VITE_USE_MOCKS === 'true'
const MOCK_BALANCE = 1000.0;
const MOCK_TRANSACTIONS = [
  { id: 1, type: 'deposit', amount: 1000, date: '2026-05-01', status: 'completed', description: 'Initial Deposit' },
  { id: 2, type: 'deposit', amount: 500,  date: '2026-05-15', status: 'completed', description: 'USDC Transfer' },
  { id: 3, type: 'withdraw', amount: -200, date: '2026-05-18', status: 'completed', description: 'Bank Withdrawal' },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export function useWallet() {
  const [balance, setBalance] = useState(USE_MOCKS ? MOCK_BALANCE : null);
  const [transactions, setTransactions] = useState(USE_MOCKS ? MOCK_TRANSACTIONS : []);
  const [walletAddress, setWalletAddress] = useState(null);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadWallet() {
      setLoading(true);
      setError(null);
      try {
        const [balanceResult, txResult] = await Promise.allSettled([
          api.wallet.balance(),
          api.wallet.transactions(),
        ]);

        if (cancelled) return;

        if (balanceResult.status === 'fulfilled') {
          const data = balanceResult.value;
          // Response shape: { data: { balance, wallet_address?, ... } } or unwrapped
          const payload = data?.data ?? data;
          if (payload?.balance != null) setBalance(Number(payload.balance));
          if (payload?.wallet_address) setWalletAddress(payload.wallet_address);
        } else {
          throw balanceResult.reason;
        }

        if (txResult.status === 'fulfilled') {
          const data = txResult.value;
          const payload = data?.data ?? data;
          const list = Array.isArray(payload?.transactions)
            ? payload.transactions
            : Array.isArray(payload?.items)
              ? payload.items
              : Array.isArray(payload)
                ? payload
                : [];
          setTransactions(list);
        }
        // A failed tx fetch is non-fatal; we still show balance.
      } catch (err) {
        if (!cancelled) {
          setError(err.message || 'Failed to load wallet');
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadWallet();
    return () => { cancelled = true; };
  }, []);

  const deposit = useCallback(async (amount, method = 'paystack') => {
    if (USE_MOCKS) {
      setBalance((b) => (b ?? 0) + amount);
      setTransactions((t) => [
        { id: Date.now(), type: 'deposit', amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: `${method} Deposit` },
        ...t,
      ]);
      return;
    }

    // Real path — let errors propagate to the caller so the UI can show them.
    const result = await api.wallet.deposit({ amount, method });
    const payload = result?.data ?? result;
    if (payload?.balance != null) {
      setBalance(Number(payload.balance));
    } else {
      setBalance((b) => (b ?? 0) + amount);
    }
    setTransactions((t) => [
      { id: Date.now(), type: 'deposit', amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: `${method} Deposit` },
      ...t,
    ]);
  }, []);

  const withdraw = useCallback(async (amount) => {
    if ((balance ?? 0) < amount) {
      throw new Error('Insufficient balance');
    }

    if (USE_MOCKS) {
      setBalance((b) => (b ?? 0) - amount);
      setTransactions((t) => [
        { id: Date.now(), type: 'withdraw', amount: -amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: 'Withdrawal' },
        ...t,
      ]);
      return;
    }

    // Real path — let errors propagate to the caller.
    const result = await api.wallet.withdraw({ amount });
    const payload = result?.data ?? result;
    if (payload?.balance != null) {
      setBalance(Number(payload.balance));
    } else {
      setBalance((b) => (b ?? 0) - amount);
    }
    setTransactions((t) => [
      { id: Date.now(), type: 'withdraw', amount: -amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: 'Withdrawal' },
      ...t,
    ]);
  }, [balance]);

  return { balance, deposit, withdraw, transactions, walletAddress, loading, error };
}
