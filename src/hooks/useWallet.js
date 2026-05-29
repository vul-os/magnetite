import { useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';

const MOCK_BALANCE = 1000.0;
const MOCK_TRANSACTIONS = [
  { id: 1, type: 'deposit', amount: 1000, date: '2026-05-01', status: 'completed', description: 'Initial Deposit' },
  { id: 2, type: 'deposit', amount: 500, date: '2026-05-15', status: 'completed', description: 'USDC Transfer' },
  { id: 3, type: 'withdraw', amount: -200, date: '2026-05-18', status: 'completed', description: 'Bank Withdrawal' },
];

export function useWallet() {
  const [balance, setBalance] = useState(MOCK_BALANCE);
  const [transactions, setTransactions] = useState(MOCK_TRANSACTIONS);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadWallet() {
      try {
        const [balanceResult, txResult] = await Promise.allSettled([
          api.wallet.balance(),
          api.wallet.transactions(),
        ]);
        if (!cancelled) {
          if (balanceResult.status === 'fulfilled' && balanceResult.value?.balance != null) {
            setBalance(balanceResult.value.balance);
          }
          if (txResult.status === 'fulfilled' && Array.isArray(txResult.value?.transactions)) {
            setTransactions(txResult.value.transactions);
          } else if (txResult.status === 'fulfilled' && Array.isArray(txResult.value)) {
            setTransactions(txResult.value);
          }
        }
      } catch { /* use mock data */ } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadWallet();
    return () => { cancelled = true; };
  }, []);

  const deposit = useCallback(async (amount, method = 'paystack') => {
    try {
      const result = await api.wallet.deposit({ amount, method });
      if (result?.balance != null) {
        setBalance(result.balance);
      } else {
        setBalance((b) => b + amount);
      }
      setTransactions((t) => [
        { id: Date.now(), type: 'deposit', amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: `${method} Deposit` },
        ...t,
      ]);
    } catch {
      // Optimistic update on failure (mock mode)
      setBalance((b) => b + amount);
      setTransactions((t) => [
        { id: Date.now(), type: 'deposit', amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: `${method} Deposit` },
        ...t,
      ]);
    }
  }, []);

  const withdraw = useCallback(async (amount) => {
    if (amount > balance) {
      throw new Error('Insufficient balance');
    }
    try {
      const result = await api.wallet.withdraw({ amount });
      if (result?.balance != null) {
        setBalance(result.balance);
      } else {
        setBalance((b) => b - amount);
      }
      setTransactions((t) => [
        { id: Date.now(), type: 'withdraw', amount: -amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: 'Withdrawal' },
        ...t,
      ]);
    } catch (err) {
      if (err.message === 'Insufficient balance') throw err;
      // Optimistic on other errors
      setBalance((b) => b - amount);
      setTransactions((t) => [
        { id: Date.now(), type: 'withdraw', amount: -amount, date: new Date().toISOString().split('T')[0], status: 'completed', description: 'Withdrawal' },
        ...t,
      ]);
    }
  }, [balance]);

  return { balance, deposit, withdraw, transactions, loading };
}
