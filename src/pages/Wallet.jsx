import { useState } from 'react';
import Layout from '../components/Layout';
import { useWallet } from '../hooks/useWallet';
import { useTranslation } from '../i18n/useTranslation';
import './Wallet.css';

const TIER_DISPLAY = {
  free: { name: 'Free' },
  basic: { name: 'Basic' },
  pro: { name: 'Pro' },
  unlimited: { name: 'Unlimited' },
};

export default function Wallet() {
  const { t } = useTranslation();
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

  const customAmountId = 'wallet-custom-amount';

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
      setDepositError(err.message || t('walletPage.depositError'));
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
          <span className="kicker">// {t('walletPage.kicker')}</span>
          <h1>{t('walletPage.title')}</h1>
          <p className="wallet-subtitle">{t('walletPage.subtitle')}</p>
        </header>

        {walletError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {walletError}
          </div>
        )}

        <div className="wallet-grid">
          <div className="wallet-left">
            {/* Subscription status */}
            <section className="sub-status-card" aria-label={t('walletPage.subscriptionLabel')}>
              <div className="sub-status-top">
                <div className="sub-tier-badge">
                  <span className="sub-tier-dot" aria-hidden="true" />
                  <span className="sub-tier-name">{tierInfo.name} {t('walletPage.plan')}</span>
                </div>
                <span className="sub-active-label" aria-label={t('walletPage.activeLabel')}>{t('walletPage.active')}</span>
              </div>
              <div className="sub-renewal">
                <span className="sub-detail-label">{t('walletPage.renews')}</span>
                <span className="sub-detail-value">{formatDate(subscription.renewalDate)}</span>
              </div>
              <div className="sub-hours">
                <div className="sub-hours-header">
                  <span className="sub-detail-label">{t('walletPage.hoursThisMonth')}</span>
                  <span className="sub-detail-value">{subscription.hoursUsed} / {subscription.hoursTotal} {t('walletPage.hrs')}</span>
                </div>
                <div className="sub-bar-track" aria-hidden="true">
                  <div
                    className="sub-bar-fill"
                    style={{ width: `${hoursPercent}%` }}
                    role="progressbar"
                    aria-valuenow={subscription.hoursUsed}
                    aria-valuemin={0}
                    aria-valuemax={subscription.hoursTotal}
                    aria-label={t('walletPage.hoursUsedLabel', { used: subscription.hoursUsed, total: subscription.hoursTotal })}
                  />
                </div>
                <span className="sub-hours-remaining">{t('walletPage.hoursRemaining', { count: hoursRemaining })}</span>
              </div>
              <button
                className="btn btn-primary btn-manage-sub"
                onClick={handleManageSubscription}
                aria-label={t('walletPage.manageSubLabel')}
              >
                {t('walletPage.manageSub')}
              </button>
            </section>

            {/* Balance card */}
            <section className="balance-card" aria-label={t('walletPage.balanceLabel')}>
              <div className="balance-card-top">
                <div className="usdc-badge">
                  <span className="usdc-icon" aria-hidden="true">$</span>
                  <span>USD</span>
                </div>
                <span className="balance-label-text">{t('walletPage.totalBalance')}</span>
              </div>
              <div className="balance-amount-row">
                <span className="balance-currency" aria-hidden="true">$</span>
                {loading
                  ? <span className="balance-loading" aria-label={t('common.loading')}>—</span>
                  : <span className="balance-value" aria-label={t('walletPage.balanceValue', { amount: balance != null ? Number(balance).toFixed(2) : '0.00' })}>{balance != null ? Number(balance).toFixed(2) : '—'}</span>
                }
              </div>
              <div className="balance-actions">
                <button className="btn btn-primary btn-add-funds" aria-label={t('walletPage.addFundsLabel')}>
                  <span aria-hidden="true">+</span> {t('walletPage.addFunds')}
                </button>
                <a href="/earnings" className="btn btn-withdraw" aria-label={t('walletPage.payoutLabel')}>
                  <span aria-hidden="true">↓</span> {t('walletPage.requestPayout')}
                </a>
              </div>
            </section>

            {/* Add funds via Paystack */}
            <section className="quick-deposit-card" aria-label={t('walletPage.addFundsSection')}>
              <span className="kicker">// {t('walletPage.addFundsKicker')}</span>
              <h3>{t('walletPage.addFundsViaPaystack')}</h3>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '1rem', fontFamily: 'var(--font-sans)' }}>
                {t('walletPage.paystackDesc')}
              </p>
              <div className="preset-amounts" role="group" aria-label={t('walletPage.presetAmounts')}>
                {[5, 10, 25, 50].map((amt) => (
                  <button
                    key={amt}
                    className={`preset-btn${selectedPreset === amt ? ' active' : ''}`}
                    onClick={() => handlePresetClick(amt)}
                    aria-pressed={selectedPreset === amt}
                    aria-label={t('walletPage.presetLabel', { amount: amt })}
                  >
                    ${amt}
                  </button>
                ))}
              </div>
              <div className="custom-amount-row">
                <div className="custom-amount-input">
                  <span className="input-prefix" aria-hidden="true">$</span>
                  <label htmlFor={customAmountId} className="sr-only">{t('walletPage.customAmountLabel')}</label>
                  <input
                    id={customAmountId}
                    type="number"
                    placeholder={t('walletPage.customAmountPlaceholder')}
                    value={customAmount}
                    onChange={handleCustomChange}
                    min="1"
                    step="1"
                    aria-label={t('walletPage.customAmountLabel')}
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
                aria-label={depositLoading ? t('walletPage.processing') : t('walletPage.depositLabel', { amount: getAmount() || '' })}
              >
                {depositLoading
                  ? t('walletPage.processing')
                  : `${t('walletPage.addFunds')} ${getAmount() ? `$${getAmount()}` : ''} ${t('walletPage.viaPaystack')}`}
              </button>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginTop: '0.75rem', fontFamily: 'var(--font-sans)' }}>
                {t('walletPage.paystackPowered')}
              </p>
            </section>
          </div>

          <div className="wallet-right">
            <section className="transactions-card" aria-label={t('walletPage.transactionsLabel')}>
              <div className="transactions-header">
                <h3>{t('walletPage.recentTransactions')}</h3>
                <a href="/earnings" className="btn-text-link">{t('walletPage.viewAll')}</a>
              </div>
              {loading ? (
                <div className="loading-state">
                  <span className="spinner" aria-hidden="true" />
                  <span>{t('walletPage.loadingTransactions')}</span>
                </div>
              ) : transactions.length === 0 ? (
                <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  {t('walletPage.noTransactions')}
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
                        <span className={`tx-amount ${tx.amount > 0 ? 'positive' : 'negative'}`} aria-label={`${tx.amount > 0 ? '+' : ''}${Number(tx.amount).toFixed(2)} USD`}>
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
            </section>
          </div>
        </div>
      </div>
    </Layout>
  );
}
