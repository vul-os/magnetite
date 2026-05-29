import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import './Wallet.css';

const MOCK_TRANSACTIONS = [
  { id: 1, type: 'deposit', amount: 500.00, date: '2026-05-18T14:32:00', status: 'completed', description: 'Paystack Deposit' },
  { id: 2, type: 'subscription', amount: -9.99, date: '2026-05-17T19:45:00', status: 'completed', description: 'Pro Plan - Monthly' },
  { id: 3, type: 'deposit', amount: 100.00, date: '2026-05-15T10:00:00', status: 'completed', description: 'USDC Transfer' },
  { id: 4, type: 'withdraw', amount: -75.00, date: '2026-05-14T16:30:00', status: 'completed', description: 'Bank Withdrawal' },
  { id: 5, type: 'subscription', amount: -9.99, date: '2026-04-17T19:45:00', status: 'completed', description: 'Pro Plan - Monthly' },
  { id: 6, type: 'deposit', amount: 200.00, date: '2026-03-15T10:00:00', status: 'completed', description: 'USDC Transfer' },
];

const WALLET_ADDRESS = '0x7a3d8c9e1f4b2a5d8e9f1a2b3c4d5e6f7a8b9c0d';

const TIER_DISPLAY = {
  free: { name: 'Free' },
  basic: { name: 'Basic' },
  pro: { name: 'Pro' },
  unlimited: { name: 'Unlimited' },
};

export default function Wallet() {
  const [balance, setBalance] = useState(127.50);
  const [transactions, setTransactions] = useState(MOCK_TRANSACTIONS);
  const [selectedPreset, setSelectedPreset] = useState(null);
  const [customAmount, setCustomAmount] = useState('');
  const [depositMethod, setDepositMethod] = useState('paystack');
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);
  const [subscription] = useState({
    tier: 'pro',
    renewalDate: '2026-06-17',
    hoursUsed: 32,
    hoursTotal: 50,
  });

  useEffect(() => {
    async function loadWallet() {
      try {
        const [balanceData, txData] = await Promise.allSettled([
          api.wallet.balance(),
          api.wallet.transactions(),
        ]);
        if (balanceData.status === 'fulfilled' && balanceData.value?.balance != null) {
          setBalance(balanceData.value.balance);
        }
        if (txData.status === 'fulfilled' && txData.value?.transactions) {
          setTransactions(txData.value.transactions);
        }
      } catch {
        /* use mock data */
      } finally {
        setLoading(false);
      }
    }
    loadWallet();
  }, []);

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

  const handleCopyAddress = async () => {
    try {
      await navigator.clipboard.writeText(WALLET_ADDRESS);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const handleDeposit = async (e) => {
    e.preventDefault();
    const amount = getAmount();
    if (!amount) return;
    try {
      await api.wallet.deposit({ amount: parseFloat(amount), method: depositMethod });
    } catch {
      /* fallback: log only */
      console.log(`Deposit $${amount} via ${depositMethod}`);
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
          <span className="kicker">// USDC WALLET</span>
          <h1>Wallet</h1>
          <p className="wallet-subtitle">Manage your USDC balance, deposits, and subscription</p>
        </header>

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
                  <span className="usdc-icon">◎</span>
                  <span>USDC</span>
                </div>
                <span className="balance-label-text">Total Balance</span>
              </div>
              <div className="balance-amount-row">
                <span className="balance-currency">$</span>
                {loading
                  ? <span className="balance-loading">—</span>
                  : <span className="balance-value">{balance.toFixed(2)}</span>
                }
              </div>
              <div className="balance-actions">
                <button className="btn btn-primary btn-add-funds">
                  <span aria-hidden="true">+</span> Add Funds
                </button>
                <button className="btn btn-withdraw">
                  <span aria-hidden="true">↓</span> Withdraw
                </button>
              </div>
              <div className="quick-actions-row">
                <button className="quick-action-item">
                  <span className="qa-icon" aria-hidden="true">↗</span>
                  <span>Send</span>
                </button>
                <button className="quick-action-item">
                  <span className="qa-icon" aria-hidden="true">↙</span>
                  <span>Receive</span>
                </button>
                <button className="quick-action-item">
                  <span className="qa-icon" aria-hidden="true">⟳</span>
                  <span>Swap</span>
                </button>
              </div>
            </div>

            {/* Quick deposit */}
            <div className="quick-deposit-card">
              <span className="kicker">// FUND YOUR ACCOUNT</span>
              <h3>Quick Deposit</h3>
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
              <div className="deposit-methods">
                <label className={`method-option${depositMethod === 'paystack' ? ' active' : ''}`}>
                  <input
                    type="radio"
                    name="method"
                    value="paystack"
                    checked={depositMethod === 'paystack'}
                    onChange={(e) => setDepositMethod(e.target.value)}
                  />
                  <div className="method-content">
                    <span className="method-icon" aria-hidden="true">🏦</span>
                    <div className="method-info">
                      <span className="method-name">Paystack</span>
                      <span className="method-desc">ZAR (South Africa)</span>
                    </div>
                  </div>
                </label>
                <label className={`method-option${depositMethod === 'usdc' ? ' active' : ''}`}>
                  <input
                    type="radio"
                    name="method"
                    value="usdc"
                    checked={depositMethod === 'usdc'}
                    onChange={(e) => setDepositMethod(e.target.value)}
                  />
                  <div className="method-content">
                    <span className="method-icon" aria-hidden="true">◎</span>
                    <div className="method-info">
                      <span className="method-name">USDC Direct</span>
                      <span className="method-desc">Deposit from wallet</span>
                    </div>
                  </div>
                </label>
              </div>
              <button
                className="btn btn-primary btn-deposit-submit"
                onClick={handleDeposit}
                disabled={!getAmount()}
              >
                Deposit {getAmount() ? `$${getAmount()}` : ''} via {depositMethod === 'paystack' ? 'Paystack' : 'USDC'}
              </button>
            </div>

            {/* Wallet address */}
            <div className="wallet-address-card">
              <span className="kicker">// DEPOSIT ADDRESS</span>
              <h3>Your Deposit Address</h3>
              <div className="address-card">
                <div className="qr-placeholder" aria-label="QR code placeholder">
                  <div className="qr-box">QR</div>
                </div>
                <div className="address-details">
                  <div className="address-network-label">Ethereum (ERC-20)</div>
                  <div className="address-value-row">
                    <code className="address-code">{WALLET_ADDRESS}</code>
                  </div>
                  <button className="btn btn-copy-addr" onClick={handleCopyAddress}>
                    {copied ? '✓ Copied!' : 'Copy Address'}
                  </button>
                </div>
              </div>
              <p className="address-notice">
                Only send USDC on Ethereum network to this address. Other tokens may be lost.
              </p>
            </div>
          </div>

          <div className="wallet-right">
            <div className="transactions-card">
              <div className="transactions-header">
                <h3>Recent Transactions</h3>
                <button className="btn-text-link">View All</button>
              </div>
              {loading ? (
                <div className="loading-state">
                  <span className="spinner" />
                  <span>Loading transactions…</span>
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
                          {tx.amount > 0 ? '+' : ''}{tx.amount.toFixed(2)}
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
