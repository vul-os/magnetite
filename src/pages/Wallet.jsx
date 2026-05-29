import { useState } from 'react';
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
  free: { name: 'Free', color: '#6b7280' },
  basic: { name: 'Basic', color: '#3b82f6' },
  pro: { name: 'Pro', color: '#8b5cf6' },
  unlimited: { name: 'Unlimited', color: '#f59e0b' },
};

export default function Wallet() {
  const [balance] = useState(127.50);
  const [selectedPreset, setSelectedPreset] = useState(null);
  const [customAmount, setCustomAmount] = useState('');
  const [depositMethod, setDepositMethod] = useState('paystack');
  const [copied, setCopied] = useState(false);
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

  const handleCopyAddress = async () => {
    try {
      await navigator.clipboard.writeText(WALLET_ADDRESS);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const handleDeposit = (e) => {
    e.preventDefault();
    const amount = getAmount();
    if (amount) {
      console.log(`Deposit ${amount} via ${depositMethod}`);
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

  return (
    <Layout>
      <div className="wallet">
        <header className="wallet-header">
          <h1>Wallet</h1>
          <p>Manage your USDC balance and subscription</p>
        </header>

        <div className="wallet-grid">
          <div className="wallet-left">
            <div className="subscription-status-card">
              <div className="subscription-card-header">
                <div className="subscription-badge" style={{ backgroundColor: tierInfo.color }}>
                  <span className="subscription-tier-name">{tierInfo.name}</span>
                </div>
                <span className="current-plan-label">Current Plan</span>
              </div>
              <div className="subscription-details">
                <div className="subscription-renewal">
                  <span className="detail-label">Renews</span>
                  <span className="detail-value">{formatDate(subscription.renewalDate)}</span>
                </div>
                <div className="subscription-hours">
                  <span className="detail-label">Hours This Month</span>
                  <div className="hours-progress">
                    <div className="hours-bar">
                      <div 
                        className="hours-fill" 
                        style={{ width: `${(subscription.hoursUsed / subscription.hoursTotal) * 100}%` }}
                      />
                    </div>
                    <span className="hours-text">{subscription.hoursUsed} / {subscription.hoursTotal} hrs used</span>
                  </div>
                  <span className="hours-remaining">{hoursRemaining} hours remaining</span>
                </div>
              </div>
              <button className="btn btn-primary btn-manage-subscription" onClick={handleManageSubscription}>
                Manage Subscription
              </button>
            </div>

            <div className="balance-card">
              <div className="balance-card-header">
                <div className="usdc-badge">
                  <span className="usdc-icon">◎</span>
                  <span>USDC</span>
                </div>
                <span className="balance-label">Total Balance</span>
              </div>
              <div className="balance-amount">
                <span className="currency">$</span>
                <span className="amount">{balance.toFixed(2)}</span>
              </div>
              <div className="balance-actions">
                <button className="btn btn-primary btn-add-funds">
                  <span>+</span> Add Funds
                </button>
                <button className="btn btn-outline">
                  <span>↓</span> Withdraw
                </button>
              </div>
              <div className="quick-actions">
                <button className="quick-action">
                  <span className="qa-icon">↗</span>
                  <span>Send</span>
                </button>
                <button className="quick-action">
                  <span className="qa-icon">↙</span>
                  <span>Receive</span>
                </button>
                <button className="quick-action">
                  <span className="qa-icon">⟳</span>
                  <span>Swap</span>
                </button>
              </div>
            </div>

            <div className="quick-deposit-section">
              <h3>Quick Deposit</h3>
              <div className="preset-amounts">
                <button
                  className={`preset-btn ${selectedPreset === 5 ? 'active' : ''}`}
                  onClick={() => handlePresetClick(5)}
                >
                  $5
                </button>
                <button
                  className={`preset-btn ${selectedPreset === 10 ? 'active' : ''}`}
                  onClick={() => handlePresetClick(10)}
                >
                  $10
                </button>
                <button
                  className={`preset-btn ${selectedPreset === 25 ? 'active' : ''}`}
                  onClick={() => handlePresetClick(25)}
                >
                  $25
                </button>
                <button
                  className={`preset-btn ${selectedPreset === 50 ? 'active' : ''}`}
                  onClick={() => handlePresetClick(50)}
                >
                  $50
                </button>
              </div>
              <div className="custom-amount-row">
                <div className="custom-amount-input">
                  <span className="input-prefix">$</span>
                  <input
                    type="number"
                    placeholder="Custom amount"
                    value={customAmount}
                    onChange={handleCustomChange}
                    min="1"
                    step="1"
                  />
                </div>
              </div>
              <div className="deposit-methods">
                <label className={`method-option ${depositMethod === 'paystack' ? 'active' : ''}`}>
                  <input
                    type="radio"
                    name="method"
                    value="paystack"
                    checked={depositMethod === 'paystack'}
                    onChange={(e) => setDepositMethod(e.target.value)}
                  />
                  <div className="method-content">
                    <span className="method-icon">🏦</span>
                    <div className="method-info">
                      <span className="method-name">Paystack</span>
                      <span className="method-desc">ZAR (South Africa)</span>
                    </div>
                  </div>
                </label>
                <label className={`method-option ${depositMethod === 'usdc' ? 'active' : ''}`}>
                  <input
                    type="radio"
                    name="method"
                    value="usdc"
                    checked={depositMethod === 'usdc'}
                    onChange={(e) => setDepositMethod(e.target.value)}
                  />
                  <div className="method-content">
                    <span className="method-icon">◎</span>
                    <div className="method-info">
                      <span className="method-name">USDC Direct</span>
                      <span className="method-desc">Deposit from wallet</span>
                    </div>
                  </div>
                </label>
              </div>
              <button
                className="btn btn-primary btn-deposit"
                onClick={handleDeposit}
                disabled={!getAmount()}
              >
                Deposit {getAmount() ? `$${getAmount()}` : ''} via {depositMethod === 'paystack' ? 'Paystack' : 'USDC'}
              </button>
            </div>

            <div className="wallet-address-section">
              <h3>Your Deposit Address</h3>
              <div className="address-card">
                <div className="qr-placeholder">
                  <div className="qr-box">
                    <span>QR</span>
                  </div>
                </div>
                <div className="address-details">
                  <div className="address-label">Ethereum (ERC-20)</div>
                  <div className="address-value">
                    <code>{WALLET_ADDRESS}</code>
                  </div>
                  <button className="btn btn-copy" onClick={handleCopyAddress}>
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
            <div className="transactions-section">
              <div className="section-header">
                <h3>Recent Transactions</h3>
                <button className="btn btn-text">View All</button>
              </div>
              <div className="transactions-list">
                {MOCK_TRANSACTIONS.map(tx => (
                  <div key={tx.id} className="transaction-item">
                    <div className={`tx-icon tx-icon-${tx.type}`}>
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
            </div>
          </div>
        </div>
      </div>
    </Layout>
  );
}