import { createContext, useContext, useMemo } from 'react';
import { useWallet as useWalletData } from '../hooks/useWallet';

/**
 * WalletContext — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
 *
 * Previously this provider simulated a custodial balance with `deposit()` and
 * `withdraw()` mutating a number. That model is gone: the node holds no funds,
 * so there is no balance to mutate. The provider now just shares the linked
 * address + signed receipts from `useWallet` so several surfaces can read them
 * without each firing its own request.
 */

const WalletContext = createContext(null);

export function WalletProvider({ children }) {
  const { address, custodial, rail, receipts, link, loading, error } = useWalletData();

  const value = useMemo(
    () => ({ address, custodial, rail, receipts, link, loading, error }),
    [address, custodial, rail, receipts, link, loading, error],
  );

  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useWallet() {
  const context = useContext(WalletContext);
  if (!context) throw new Error('useWallet must be used within WalletProvider');
  return context;
}
