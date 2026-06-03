import { createContext, useContext, useState } from 'react';

const WalletContext = createContext();

export function WalletProvider({ children }) {
  const [balance, setBalance] = useState(100);
  // Lazy initializer keeps the impure Date.now() call out of render.
  const [transactions, setTransactions] = useState(() => [
    { id: 1, type: 'deposit', amount: 100, currency: 'USD', timestamp: Date.now() - 86400000, status: 'completed' },
  ]);
  const [isLoading, setIsLoading] = useState(false);

  const deposit = async (amount) => {
    setIsLoading(true);
    await new Promise(r => setTimeout(r, 500));
    const newTx = { id: Date.now(), type: 'deposit', amount, currency: 'USD', timestamp: Date.now(), status: 'completed' };
    setTransactions(prev => [newTx, ...prev]);
    setBalance(prev => prev + amount);
    setIsLoading(false);
    return { success: true };
  };

  const withdraw = async (amount) => {
    if (amount > balance) return { success: false, error: 'Insufficient balance' };
    setIsLoading(true);
    await new Promise(r => setTimeout(r, 500));
    const newTx = { id: Date.now(), type: 'withdraw', amount, currency: 'USD', timestamp: Date.now(), status: 'completed' };
    setTransactions(prev => [newTx, ...prev]);
    setBalance(prev => prev - amount);
    setIsLoading(false);
    return { success: true };
  };

  return (
    <WalletContext.Provider value={{ balance, transactions, isLoading, deposit, withdraw }}>
      {children}
    </WalletContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useWallet() {
  const context = useContext(WalletContext);
  if (!context) throw new Error('useWallet must be used within WalletProvider');
  return context;
}
