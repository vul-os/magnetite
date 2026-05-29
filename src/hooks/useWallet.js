import { useState, useCallback } from 'react';

export function useWallet() {
  const [balance, setBalance] = useState(1000.0);
  const [transactions, setTransactions] = useState([
    { id: 1, type: 'deposit', amount: 1000, date: '2026-05-01', status: 'completed' },
    { id: 2, type: 'deposit', amount: 500, date: '2026-05-15', status: 'completed' },
    { id: 3, type: 'withdraw', amount: 200, date: '2026-05-18', status: 'completed' },
  ]);

  const deposit = useCallback((amount) => {
    return new Promise((resolve) => {
      setTimeout(() => {
        setBalance((b) => b + amount);
        setTransactions((t) => [
          { id: Date.now(), type: 'deposit', amount, date: new Date().toISOString().split('T')[0], status: 'completed' },
          ...t,
        ]);
        resolve();
      }, 300);
    });
  }, []);

  const withdraw = useCallback((amount) => {
    return new Promise((resolve, reject) => {
      setTimeout(() => {
        if (amount > balance) {
          reject(new Error('Insufficient balance'));
          return;
        }
        setBalance((b) => b - amount);
        setTransactions((t) => [
          { id: Date.now(), type: 'withdraw', amount, date: new Date().toISOString().split('T')[0], status: 'completed' },
          ...t,
        ]);
        resolve();
      }, 300);
    });
  }, [balance]);

  return { balance, deposit, withdraw, transactions };
}
