import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import './Earnings.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Mock data — only used when VITE_USE_MOCKS === 'true'
const MOCK_TRANSACTIONS = [
  { id: 'tx_001', type: 'game',    description: 'Cosmic Raiders - Session #4521', amount: -1.50,   balance: 24580.50, date: '2026-05-18 14:32' },
  { id: 'tx_002', type: 'deposit', description: 'USDC Deposit',                    amount: 500.00,  balance: 24582.50, date: '2026-05-18 10:15' },
  { id: 'tx_003', type: 'payout',  description: 'Payout to Wallet',                amount: -500.00, balance: 24083.50, date: '2026-05-17 18:00' },
];

const MOCK_PAYOUTS = [
  { id: 'pay_001', amount: 500.00,  method: 'USDC (Polygon)', status: 'Completed', date: '2026-05-17' },
  { id: 'pay_002', amount: 1250.00, method: 'USDC (Polygon)', status: 'Completed', date: '2026-05-10' },
];

export default function Earnings() {
  const [balance, setBalance]               = useState(USE_MOCKS ? 24580.50 : null);
  const [pendingBalance, setPendingBalance]  = useState(USE_MOCKS ? 384.25 : 0);
  const [lifetimeEarnings, setLifetimeEarnings] = useState(USE_MOCKS ? 89432.00 : null);
  const [transactions, setTransactions]     = useState(USE_MOCKS ? MOCK_TRANSACTIONS : []);
  const [payouts, setPayouts]               = useState(USE_MOCKS ? MOCK_PAYOUTS : []);
  const [activeTab, setActiveTab]           = useState('transactions');
  const [withdrawing, setWithdrawing]       = useState(false);
  const [withdrawAmount, setWithdrawAmount] = useState('');
  const [withdrawSuccess, setWithdrawSuccess] = useState(false);
  const [withdrawError, setWithdrawError]   = useState(null);
  const [loading, setLoading]               = useState(!USE_MOCKS);
  const [loadError, setLoadError]           = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadData() {
      setLoading(true);
      setLoadError(null);
      try {
        const [earningsData, txData, payoutsData] = await Promise.allSettled([
          api.developer.earnings(),
          api.wallet.transactions(),
          api.developer.payouts(),
        ]);

        if (cancelled) return;

        if (earningsData.status === 'fulfilled') {
          const d = earningsData.value?.data ?? earningsData.value;
          // EarningsSummary: { total_earnings, pending_payout, total_paid, recent_earnings }
          if (d?.total_earnings != null) setLifetimeEarnings(Number(d.total_earnings));
          if (d?.pending_payout != null) setPendingBalance(Number(d.pending_payout));
          // Available = total_paid (already paid out) used here as available balance
          if (d?.total_paid != null) setBalance(Number(d.total_paid));
        } else {
          throw earningsData.reason;
        }

        if (txData.status === 'fulfilled') {
          const d = txData.value?.data ?? txData.value;
          const list = Array.isArray(d?.transactions) ? d.transactions
            : Array.isArray(d?.items) ? d.items
            : Array.isArray(d) ? d : [];
          setTransactions(list);
        }

        if (payoutsData.status === 'fulfilled') {
          const d = payoutsData.value?.data ?? payoutsData.value;
          const list = Array.isArray(d) ? d : (d?.payouts ?? []);
          setPayouts(list);
        }
      } catch (err) {
        if (!cancelled) setLoadError(err.message || 'Failed to load earnings');
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadData();
    return () => { cancelled = true; };
  }, []);

  const handleWithdraw = async (e) => {
    e.preventDefault();
    if (!withdrawAmount || parseFloat(withdrawAmount) <= 0) return;

    setWithdrawing(true);
    setWithdrawError(null);
    try {
      await api.wallet.withdraw({ amount: parseFloat(withdrawAmount) });
      setBalance(prev => (prev ?? 0) - parseFloat(withdrawAmount));
      setWithdrawSuccess(true);
      setWithdrawAmount('');
      setTimeout(() => setWithdrawSuccess(false), 3000);
    } catch (err) {
      setWithdrawError(err.message || 'Withdrawal failed. Please try again.');
    } finally {
      setWithdrawing(false);
    }
  };

  const formatAmount = (amount) => {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(Math.abs(amount));
  };

  return (
    <Layout>
      <div className="earnings-page">
        <header className="earnings-header">
          <span className="kicker">// DEVELOPER EARNINGS</span>
          <h1>Earnings</h1>
          <p className="earnings-subtitle">Track your Rust game revenue and manage USDC payouts</p>
        </header>

        {loadError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {loadError}
          </div>
        )}

        <div className="earnings-summary">
          <div className="summary-card primary">
            <span className="summary-icon" aria-hidden="true">💰</span>
            <div className="summary-content">
              <span className="summary-label">Available Balance</span>
              <span className="summary-value amber">
                {loading ? '—' : balance != null ? `$${Number(balance).toLocaleString()}` : '—'}
              </span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">⏳</span>
            <div className="summary-content">
              <span className="summary-label">Pending</span>
              <span className="summary-value amber">${pendingBalance.toLocaleString()}</span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">📈</span>
            <div className="summary-content">
              <span className="summary-label">Lifetime Earnings</span>
              <span className="summary-value amber">
                {loading ? '—' : lifetimeEarnings != null ? `$${Number(lifetimeEarnings).toLocaleString()}` : '—'}
              </span>
            </div>
          </div>
        </div>

        <div className="withdraw-section">
          <span className="kicker">// USDC PAYOUT</span>
          <h3>Withdraw Earnings</h3>
          <form className="withdraw-form" onSubmit={handleWithdraw}>
            <div className="withdraw-input-group">
              <input
                type="number"
                step="0.01"
                min="1"
                max={balance}
                placeholder="Enter amount"
                value={withdrawAmount}
                onChange={(e) => setWithdrawAmount(e.target.value)}
                disabled={withdrawing}
                aria-label="Withdrawal amount"
              />
              <span className="currency-label">USDC</span>
            </div>
            <button
              type="submit"
              className="btn btn-primary withdraw-btn"
              disabled={withdrawing || !withdrawAmount || parseFloat(withdrawAmount) > (balance ?? 0)}
            >
              {withdrawing
                ? 'Processing…'
                : withdrawSuccess
                  ? '✓ Withdrawal Initiated!'
                  : 'Withdraw'}
            </button>
            {withdrawError && (
              <p role="alert" style={{ color: 'var(--color-error)', fontSize: '0.875rem', marginTop: '0.5rem' }}>
                {withdrawError}
              </p>
            )}
          </form>
          <p className="withdraw-note">Withdrawals are processed to your connected Polygon wallet within 24 hours. Platform fee: 15%.</p>
        </div>

        <div className="earnings-tabs" role="tablist">
          <button
            role="tab"
            aria-selected={activeTab === 'transactions'}
            className={`tab-btn ${activeTab === 'transactions' ? 'active' : ''}`}
            onClick={() => setActiveTab('transactions')}
          >
            Transaction History
          </button>
          <button
            role="tab"
            aria-selected={activeTab === 'payouts'}
            className={`tab-btn ${activeTab === 'payouts' ? 'active' : ''}`}
            onClick={() => setActiveTab('payouts')}
          >
            Payout History
          </button>
        </div>

        <div className="tab-content" role="tabpanel">
          {activeTab === 'transactions' ? (
            <div className="transactions-section">
              {loading ? (
                <div className="loading-state">
                  <span className="spinner large" />
                  <span>Loading transactions…</span>
                </div>
              ) : (
                <table className="transactions-table">
                  <thead>
                    <tr>
                      <th>Date</th>
                      <th>Description</th>
                      <th>Amount</th>
                      <th>Balance</th>
                    </tr>
                  </thead>
                  <tbody>
                    {transactions.map(tx => (
                      <tr key={tx.id}>
                        <td className="date-cell">{tx.date}</td>
                        <td>
                          <div className="tx-description">
                            <span className={`tx-type-icon ${tx.type}`} aria-hidden="true" />
                            {tx.description}
                          </div>
                        </td>
                        <td className={`amount-cell ${tx.amount > 0 ? 'positive' : 'negative'}`}>
                          {tx.amount > 0 ? '+' : ''}{formatAmount(tx.amount)}
                        </td>
                        <td className="balance-cell">{formatAmount(tx.balance)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ) : (
            <div className="payouts-section">
              {loading ? (
                <div className="loading-state">
                  <span className="spinner large" />
                  <span>Loading payouts…</span>
                </div>
              ) : (
                <table className="payouts-table">
                  <thead>
                    <tr>
                      <th>Date</th>
                      <th>Amount</th>
                      <th>Method</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {payouts.map(payout => (
                      <tr key={payout.id}>
                        <td className="date-cell">{payout.date}</td>
                        <td className="amount-cell positive">{formatAmount(payout.amount)}</td>
                        <td>{payout.method}</td>
                        <td><span className="status-badge completed">{payout.status}</span></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>
      </div>
    </Layout>
  );
}
