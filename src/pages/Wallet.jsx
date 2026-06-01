import { useState } from 'react';
import Layout from '../components/Layout';
import { useWallet } from '../hooks/useWallet';
import './Wallet.css';

const TIER_DISPLAY = {
  free: { name: 'Free' },
  basic: { name: 'Basic' },
  pro: { name: 'Pro' },
  unlimited: { name: 'Unlimited' },
};

export default function Wallet() {
  const { balance, transactions, loading, error: walletError, deposit: hookDeposit } = useWallet();
  const [selectedPreset, setSelectedPreset] = useState(null);
  const [customAmount, setCustomAmount] = useState('');
  const [depositError, setDepositError] = useState(null);
  const [depositLoading, setDepositLoading] = useState(false);
  const [subscription] = useState({
    tier: 'pro',
    renewalDate: '2026-06-17',
    hoursUsed: 32,
    hoursTotal: 50,
  });

  const formatDate = (dateStr) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  };

  const formatTime = (dateStr) => {
    const date = new Date(dateStr);
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
  };

  const handlePresetClick = (amount) => {
    setSelectedPreset(amount);
    setCustomAmount('');
  };

  const handleCustomChange = (e) => {
    setCustomAmount(e.target.value);
    setSelectedPreset(null);
  };

  const getAmount = () => {
    return customAmount || (selectedPreset ? selectedPreset.toString() : '');
  };

  const handleDeposit = async (e) => {
    e.preventDefault();
    const amount = getAmount();
    if (!amount) return;
    setDepositError(null);
    setDepositLoading(true);
    try {
      await hookDeposit(parseFloat(amount), 'paystack');
    } catch (err) {
      setDepositError(err.message || 'Deposit failed. Please try again.');
    } finally {
      setDepositLoading(false);
    }
  };

  const handleManageSubscription = () => {
    window.location.href = '/subscription/manage';
  };

  const getTransactionIcon = (type) => {
    switch (type) {
      case 'deposit': return '+';
      case 'withdraw': return '↓';
      case 'subscription': return '⟳';
      default: return '•';
    }
  };

  const getStatusClass = (status) => {
    switch (status) {
      case 'completed': return 'status-success';
      case 'pending': return 'status-pending';
      case 'failed': return 'status-failed';
      default: return '';
    }
  };

  const tierInfo = TIER_DISPLAY[subscription.tier] || TIER_DISPLAY.free;
  const hoursRemaining = subscription.hoursTotal - subscription.hoursUsed;
  const hoursPercent = Math.round((subscription.hoursUsed / subscription.hoursTotal) * 100);

  return (
    <Layout>
      <div className="wallet">
        <header className="wallet-page-header">
          <span className="kicker">// USD WALLET</span>
          <h1>Wallet</h1>
          <p className="wallet-subtitle">Manage your USD balance, add funds via Paystack, and manage your subscription</p>
        </header>

        {walletError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {walletError}
          </div>
        )}

        <div className="wallet-grid">
          <div className="wallet-left">
            {/* Subscription status */}
            <div className="sub-status-card">
              <div className="sub-status-top">
                <div className="sub-tier-badge">
                  <span className="sub-tier-dot" />
                  <span className="sub-tier-name">{tierInfo.name} Plan</span>
                </div>
                <span className="sub-active-label">ACTIVE</span>
              </div>
              <div className="sub-renewal">
                <span className="sub-detail-label">Renews</span>
                <span className="sub-detail-value">{formatDate(subscription.renewalDate)}</span>
              </div>
              <div className="sub-hours">
                <div className="sub-hours-header">
                  <span className="sub-detail-label">Hours This Month</span>
                  <span className="sub-detail-value">{subscription.hoursUsed} / {subscription.hoursTotal} hrs</span>
                </div>
                <div className="sub-bar-track">
                  <div
                    className="sub-bar-fill"
                    style={{ width: `${hoursPercent}%` }}
                    role="progressbar"
                    aria-valuenow={subscription.hoursUsed}
                    aria-valuemin={0}
                    aria-valuemax={subscription.hoursTotal}
                  />
                </div>
                <span className="sub-hours-remaining">{hoursRemaining} hours remaining</span>
              </div>
              <button className="btn btn-primary btn-manage-sub" onClick={handleManageSubscription}>
                Manage Subscription
              </button>
            </div>

            {/* Balance card */}
            <div className="balance-card">
              <div className="balance-card-top">
                <div className="usdc-badge">
                  <span className="usdc-icon">$</span>
                  <span>USD</span>
                </div>
                <span className="balance-label-text">Total Balance</span>
              </div>
              <div className="balance-amount-row">
                <span className="balance-currency">$</span>
                {loading
                  ? <span className="balance-loading">—</span>
                  : <span className="balance-value">{balance != null ? Number(balance).toFixed(2) : '—'}</span>
                }
              </div>
              <div className="balance-actions">
                <button className="btn btn-primary btn-add-funds">
                  <span aria-hidden="true">+</span> Add Funds
                </button>
                <a href="/earnings" className="btn btn-withdraw">
                  <span aria-hidden="true">↓</span> Request Payout
                </a>
              </div>
            </div>

            {/* Add funds via Paystack */}
            <div className="quick-deposit-card">
              <span className="kicker">// ADD FUNDS</span>
              <h3>Add Funds via Paystack</h3>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '1rem', fontFamily: 'var(--font-sans)' }}>
                Deposit using your card or bank account. Funds are added to your USD balance instantly.
              </p>
              <div className="preset-amounts">
                {[5, 10, 25, 50].map((amt) => (
                  <button
                    key={amt}
                    className={`preset-btn${selectedPreset === amt ? ' active' : ''}`}
                    onClick={() => handlePresetClick(amt)}
                  >
                    ${amt}
                  </button>
                ))}
              </div>
              <div className="custom-amount-row">
                <div className="custom-amount-input">
                  <span className="input-prefix" aria-hidden="true">$</span>
                  <input
                    type="number"
                    placeholder="Custom amount"
                    value={customAmount}
                    onChange={handleCustomChange}
                    min="1"
                    step="1"
                    aria-label="Custom deposit amount"
                  />
                </div>
              </div>
              {depositError && (
                <p style={{ color: 'var(--color-error)', fontSize: '0.875rem', marginTop: '0.5rem' }} role="alert">
                  {depositError}
                </p>
              )}
              <button
                className="btn btn-primary btn-deposit-submit"
                onClick={handleDeposit}
                disabled={!getAmount() || depositLoading}
              >
                {depositLoading
                  ? 'Processing…'
                  : `Add Funds ${getAmount() ? `$${getAmount()}` : ''} via Paystack`}
              </button>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginTop: '0.75rem', fontFamily: 'var(--font-sans)' }}>
                Powered by Paystack. Supports card and bank transfer payments.
              </p>
            </div>
          </div>

          <div className="wallet-right">
            <div className="transactions-card">
              <div className="transactions-header">
                <h3>Recent Transactions</h3>
                <a href="/earnings" className="btn-text-link">View All</a>
              </div>
              {loading ? (
                <div className="loading-state">
                  <span className="spinner" />
                  <span>Loading transactions…</span>
                </div>
              ) : transactions.length === 0 ? (
                <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  No transactions yet. Add funds to get started.
                </div>
              ) : (
                <div className="transactions-list">
                  {transactions.map(tx => (
                    <div key={tx.id} className="transaction-item">
                      <div className={`tx-icon tx-icon-${tx.type}`} aria-hidden="true">
                        {getTransactionIcon(tx.type)}
                      </div>
                      <div className="tx-details">
                        <span className="tx-description">{tx.description}</span>
                        <span className="tx-meta">
                          {formatDate(tx.date)} · {formatTime(tx.date)}
                        </span>
                      </div>
                      <div className="tx-right">
                        <span className={`tx-amount ${tx.amount > 0 ? 'positive' : 'negative'}`}>
                          {tx.amount > 0 ? '+' : ''}{Number(tx.amount).toFixed(2)}
                        </span>
                        <span className={`tx-status ${getStatusClass(tx.status)}`}>
                          {tx.status}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </Layout>
  );
}
