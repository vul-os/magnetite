import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './Earnings.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Mock data — only used when VITE_USE_MOCKS === 'true'
const MOCK_TRANSACTIONS = [
  { id: 'tx_001', type: 'game',    description: 'Cosmic Raiders - Session #4521', amount: -1.50,   balance: 24580.50, date: '2026-05-18 14:32' },
  { id: 'tx_002', type: 'deposit', description: 'Paystack Deposit',                amount: 500.00,  balance: 24582.50, date: '2026-05-18 10:15' },
  { id: 'tx_003', type: 'payout',  description: 'Wise Payout',                     amount: -500.00, balance: 24083.50, date: '2026-05-17 18:00' },
];

const MOCK_PAYOUTS = [
  { id: 'pay_001', amount: 500.00,  method: 'Wise (Bank Transfer)', status: 'Completed', date: '2026-05-17' },
  { id: 'pay_002', amount: 1250.00, method: 'Wise (Bank Transfer)', status: 'Completed', date: '2026-05-10' },
];

const MOCK_WISE_RECIPIENT = null; // no recipient in mock — user must set one

export default function Earnings() {
  const { t } = useTranslation();
  const [balance, setBalance]               = useState(USE_MOCKS ? 24580.50 : null);
  const [pendingBalance, setPendingBalance]  = useState(USE_MOCKS ? 384.25 : 0);
  const [lifetimeEarnings, setLifetimeEarnings] = useState(USE_MOCKS ? 89432.00 : null);
  const [transactions, setTransactions]     = useState(USE_MOCKS ? MOCK_TRANSACTIONS : []);
  const [payouts, setPayouts]               = useState(USE_MOCKS ? MOCK_PAYOUTS : []);
  const [activeTab, setActiveTab]           = useState('transactions');
  const [loading, setLoading]               = useState(!USE_MOCKS);
  const [loadError, setLoadError]           = useState(null);

  // ── Wise recipient state ────────────────────────────────────────────────
  const [wiseRecipient, setWiseRecipient]           = useState(USE_MOCKS ? MOCK_WISE_RECIPIENT : null);
  const [wiseLoading, setWiseLoading]               = useState(!USE_MOCKS);
  const [showRecipientForm, setShowRecipientForm]   = useState(false);
  const [recipientForm, setRecipientForm]           = useState({
    account_holder_name: '',
    currency: 'USD',
    type: 'email',
    details: { email: '' },
  });
  const [recipientSaving, setRecipientSaving]       = useState(false);
  const [recipientError, setRecipientError]         = useState(null);
  const [recipientSuccess, setRecipientSuccess]     = useState(false);

  // ── Payout request state ────────────────────────────────────────────────
  const [withdrawAmount, setWithdrawAmount]         = useState('');
  const [withdrawing, setWithdrawing]               = useState(false);
  const [withdrawSuccess, setWithdrawSuccess]       = useState(false);
  const [withdrawError, setWithdrawError]           = useState(null);
  const [payoutStatuses, setPayoutStatuses]         = useState([]);

  // Form field IDs for label-for association
  const holderNameId = 'wise-account-holder-name';
  const currencyId   = 'wise-currency';
  const emailId      = 'wise-email';
  const ibanId       = 'wise-iban';
  const bicId        = 'wise-bic';
  const accountNumId = 'wise-account-number';
  const routingNumId = 'wise-routing-number';
  const withdrawId   = 'earnings-withdraw-amount';

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadData() {
      setLoading(true);
      setWiseLoading(true);
      setLoadError(null);
      try {
        const [earningsData, txData, payoutsData, recipientData, statusData] = await Promise.allSettled([
          api.developer.earnings(),
          api.wallet.transactions(),
          api.developer.payouts(),
          api.developer.getWiseRecipient(),
          api.developer.payoutStatus(),
        ]);

        if (cancelled) return;

        if (earningsData.status === 'fulfilled') {
          const d = earningsData.value?.data ?? earningsData.value;
          if (d?.total_earnings != null) setLifetimeEarnings(Number(d.total_earnings));
          if (d?.pending_payout != null) setPendingBalance(Number(d.pending_payout));
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

        if (recipientData.status === 'fulfilled') {
          const d = recipientData.value?.data ?? recipientData.value;
          setWiseRecipient(d ?? null);
        }
        // Recipient fetch 404 = not set yet; that's fine — form is shown.

        if (statusData.status === 'fulfilled') {
          const d = statusData.value?.data ?? statusData.value;
          const list = Array.isArray(d) ? d : (d?.payouts ?? []);
          setPayoutStatuses(list);
        }
      } catch (err) {
        if (!cancelled) setLoadError(err.message || t('earnings.loadError'));
      } finally {
        if (!cancelled) {
          setLoading(false);
          setWiseLoading(false);
        }
      }
    }

    loadData();
    return () => { cancelled = true; };
  }, [t]);

  // ── Wise recipient form handlers ────────────────────────────────────────
  const handleRecipientTypeChange = (type) => {
    const defaultDetails = type === 'email' ? { email: '' }
      : type === 'iban' ? { iban: '', bic: '' }
      : type === 'ach' ? { account_number: '', routing_number: '' }
      : {};
    setRecipientForm(f => ({ ...f, type, details: defaultDetails }));
    setRecipientError(null);
  };

  const handleRecipientDetailChange = (key, value) => {
    setRecipientForm(f => ({ ...f, details: { ...f.details, [key]: value } }));
  };

  const handleSaveRecipient = async (e) => {
    e.preventDefault();
    setRecipientSaving(true);
    setRecipientError(null);
    setRecipientSuccess(false);
    try {
      const saved = await api.developer.saveWiseRecipient(recipientForm);
      const d = saved?.data ?? saved;
      setWiseRecipient(d ?? recipientForm);
      setRecipientSuccess(true);
      setShowRecipientForm(false);
      setTimeout(() => setRecipientSuccess(false), 3000);
    } catch (err) {
      setRecipientError(err.message || t('earnings.saveRecipientError'));
    } finally {
      setRecipientSaving(false);
    }
  };

  const handleDeleteRecipient = async () => {
    if (!window.confirm(t('earnings.removeRecipientConfirm'))) return;
    try {
      await api.developer.deleteWiseRecipient();
      setWiseRecipient(null);
      setShowRecipientForm(false);
    } catch (err) {
      setRecipientError(err.message || t('earnings.removeRecipientError'));
    }
  };

  // ── Payout request handler ──────────────────────────────────────────────
  const handleWithdraw = async (e) => {
    e.preventDefault();
    if (!withdrawAmount || parseFloat(withdrawAmount) <= 0) return;
    if (!wiseRecipient) {
      setWithdrawError(t('earnings.noRecipientError'));
      return;
    }

    setWithdrawing(true);
    setWithdrawError(null);
    try {
      await api.developer.requestPayout({ amount: parseFloat(withdrawAmount) });
      setBalance(prev => (prev ?? 0) - parseFloat(withdrawAmount));
      setWithdrawSuccess(true);
      setWithdrawAmount('');
      // Refresh payout status list
      const statusData = await api.developer.payoutStatus().catch(() => null);
      if (statusData) {
        const d = statusData?.data ?? statusData;
        setPayoutStatuses(Array.isArray(d) ? d : (d?.payouts ?? []));
      }
      setTimeout(() => setWithdrawSuccess(false), 5000);
    } catch (err) {
      setWithdrawError(err.message || t('earnings.payoutError'));
    } finally {
      setWithdrawing(false);
    }
  };

  const formatAmount = (amount) => {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(Math.abs(amount));
  };

  const recipientLabel = (r) => {
    if (!r) return '';
    const name = r.account_holder_name ?? r.name ?? '';
    const type = r.type ?? '';
    const detail = r.details?.email ?? r.details?.iban ?? r.details?.account_number ?? '';
    return [name, type, detail].filter(Boolean).join(' · ');
  };

  return (
    <Layout>
      <div className="earnings-page">
        <header className="earnings-header">
          <span className="kicker">// {t('earnings.kicker')}</span>
          <h1>{t('earnings.title')}</h1>
          <p className="earnings-subtitle">{t('earnings.subtitle')}</p>
        </header>

        {loadError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {loadError}
          </div>
        )}

        <div className="earnings-summary" aria-label={t('earnings.summaryLabel')}>
          <div className="summary-card primary">
            <span className="summary-icon" aria-hidden="true">💰</span>
            <div className="summary-content">
              <span className="summary-label">{t('earnings.availableBalance')}</span>
              <span className="summary-value amber" aria-live="polite">
                {loading ? '—' : balance != null ? `$${Number(balance).toLocaleString()}` : '—'}
              </span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">⏳</span>
            <div className="summary-content">
              <span className="summary-label">{t('earnings.pending')}</span>
              <span className="summary-value amber">${pendingBalance.toLocaleString()}</span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">📈</span>
            <div className="summary-content">
              <span className="summary-label">{t('earnings.lifetimeEarnings')}</span>
              <span className="summary-value amber" aria-live="polite">
                {loading ? '—' : lifetimeEarnings != null ? `$${Number(lifetimeEarnings).toLocaleString()}` : '—'}
              </span>
            </div>
          </div>
        </div>

        {/* ── Wise Recipient Section ─────────────────────────────────────── */}
        <div className="withdraw-section">
          <span className="kicker">// {t('earnings.wiseKicker')}</span>
          <h3>{t('earnings.payoutRecipient')}</h3>
          <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '1rem', fontFamily: 'var(--font-sans)' }}>
            {t('earnings.wiseDesc')}
          </p>

          {wiseLoading ? (
            <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>{t('common.loading')}</p>
          ) : wiseRecipient && !showRecipientForm ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', flexWrap: 'wrap', padding: '0.75rem 1rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)', marginBottom: '1rem' }}>
              <span style={{ fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', color: 'var(--color-text-primary)' }}>
                {recipientLabel(wiseRecipient)}
              </span>
              {recipientSuccess && (
                <span style={{ color: 'var(--color-success)', fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)' }} role="status">✓ {t('earnings.saved')}</span>
              )}
              <div style={{ marginLeft: 'auto', display: 'flex', gap: '0.5rem' }}>
                <button className="btn btn-secondary" style={{ fontSize: 'var(--text-xs)' }} onClick={() => { setShowRecipientForm(true); setRecipientError(null); }}>
                  {t('common.edit')}
                </button>
                <button className="btn" style={{ fontSize: 'var(--text-xs)', color: 'var(--color-error)', border: '1px solid var(--color-error)' }} onClick={handleDeleteRecipient} aria-label={t('earnings.removeRecipient')}>
                  {t('earnings.remove')}
                </button>
              </div>
            </div>
          ) : !showRecipientForm ? (
            <button className="btn btn-secondary" style={{ marginBottom: '1rem' }} onClick={() => setShowRecipientForm(true)}>
              + {t('earnings.addRecipient')}
            </button>
          ) : null}

          {showRecipientForm && (
            <form onSubmit={handleSaveRecipient} aria-label={t('earnings.recipientFormLabel')} style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem', marginBottom: '1.5rem', padding: '1rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)' }}>
              <fieldset style={{ border: 'none', padding: 0, margin: 0 }}>
                <legend style={{ fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)', color: 'var(--color-text-muted)', marginBottom: '0.5rem' }}>
                  {t('earnings.recipientType')}
                </legend>
                <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                  {['email', 'iban', 'ach'].map(type => (
                    <label key={type} style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', cursor: 'pointer', fontSize: 'var(--text-sm)' }}>
                      <input
                        type="radio"
                        name="recipientType"
                        value={type}
                        checked={recipientForm.type === type}
                        onChange={() => handleRecipientTypeChange(type)}
                      />
                      {type.toUpperCase()}
                    </label>
                  ))}
                </div>
              </fieldset>

              <div>
                <label htmlFor={holderNameId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                  {t('earnings.accountHolderName')}
                </label>
                <input
                  id={holderNameId}
                  className="input"
                  type="text"
                  placeholder={t('earnings.accountHolderName')}
                  value={recipientForm.account_holder_name}
                  onChange={e => setRecipientForm(f => ({ ...f, account_holder_name: e.target.value }))}
                  required
                />
              </div>

              <div>
                <label htmlFor={currencyId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                  {t('earnings.currency')}
                </label>
                <input
                  id={currencyId}
                  className="input"
                  type="text"
                  placeholder={t('earnings.currencyPlaceholder')}
                  value={recipientForm.currency}
                  onChange={e => setRecipientForm(f => ({ ...f, currency: e.target.value.toUpperCase() }))}
                  required
                  style={{ textTransform: 'uppercase' }}
                />
              </div>

              {recipientForm.type === 'email' && (
                <div>
                  <label htmlFor={emailId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                    {t('earnings.wiseEmail')}
                  </label>
                  <input
                    id={emailId}
                    className="input"
                    type="email"
                    placeholder={t('earnings.wiseEmail')}
                    value={recipientForm.details.email ?? ''}
                    onChange={e => handleRecipientDetailChange('email', e.target.value)}
                    required
                  />
                </div>
              )}
              {recipientForm.type === 'iban' && (
                <>
                  <div>
                    <label htmlFor={ibanId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                      {t('earnings.iban')}
                    </label>
                    <input
                      id={ibanId}
                      className="input"
                      type="text"
                      placeholder="IBAN"
                      value={recipientForm.details.iban ?? ''}
                      onChange={e => handleRecipientDetailChange('iban', e.target.value)}
                      required
                    />
                  </div>
                  <div>
                    <label htmlFor={bicId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                      {t('earnings.bic')}
                    </label>
                    <input
                      id={bicId}
                      className="input"
                      type="text"
                      placeholder="BIC / SWIFT"
                      value={recipientForm.details.bic ?? ''}
                      onChange={e => handleRecipientDetailChange('bic', e.target.value)}
                    />
                  </div>
                </>
              )}
              {recipientForm.type === 'ach' && (
                <>
                  <div>
                    <label htmlFor={accountNumId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                      {t('earnings.accountNumber')}
                    </label>
                    <input
                      id={accountNumId}
                      className="input"
                      type="text"
                      placeholder={t('earnings.accountNumber')}
                      value={recipientForm.details.account_number ?? ''}
                      onChange={e => handleRecipientDetailChange('account_number', e.target.value)}
                      required
                    />
                  </div>
                  <div>
                    <label htmlFor={routingNumId} style={{ display: 'block', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.25rem', fontFamily: 'var(--font-mono)' }}>
                      {t('earnings.routingNumber')}
                    </label>
                    <input
                      id={routingNumId}
                      className="input"
                      type="text"
                      placeholder={t('earnings.routingNumber')}
                      value={recipientForm.details.routing_number ?? ''}
                      onChange={e => handleRecipientDetailChange('routing_number', e.target.value)}
                      required
                    />
                  </div>
                </>
              )}

              {recipientError && (
                <p role="alert" style={{ color: 'var(--color-error)', fontSize: '0.875rem' }}>{recipientError}</p>
              )}

              <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.25rem' }}>
                <button type="submit" className="btn btn-primary" disabled={recipientSaving}>
                  {recipientSaving ? t('earnings.saving') : t('earnings.saveRecipient')}
                </button>
                <button type="button" className="btn btn-secondary" onClick={() => { setShowRecipientForm(false); setRecipientError(null); }}>
                  {t('common.cancel')}
                </button>
              </div>
            </form>
          )}
        </div>

        {/* ── Payout Request Section ─────────────────────────────────────── */}
        <div className="withdraw-section" style={{ marginTop: '2rem' }}>
          <span className="kicker">// {t('earnings.requestPayoutKicker')}</span>
          <h3>{t('earnings.requestPayout')}</h3>
          <form className="withdraw-form" onSubmit={handleWithdraw} aria-label={t('earnings.payoutFormLabel')}>
            <div className="withdraw-input-group">
              <label htmlFor={withdrawId} className="sr-only">{t('earnings.payoutAmount')}</label>
              <input
                id={withdrawId}
                type="number"
                step="0.01"
                min="1"
                max={balance ?? undefined}
                placeholder={t('earnings.payoutAmountPlaceholder')}
                value={withdrawAmount}
                onChange={(e) => setWithdrawAmount(e.target.value)}
                disabled={withdrawing}
                aria-label={t('earnings.payoutAmount')}
              />
              <span className="currency-label" aria-hidden="true">USD</span>
            </div>
            <button
              type="submit"
              className="btn btn-primary withdraw-btn"
              disabled={withdrawing || !withdrawAmount || parseFloat(withdrawAmount) > (balance ?? 0) || !wiseRecipient}
            >
              {withdrawing
                ? t('earnings.processing')
                : withdrawSuccess
                  ? `✓ ${t('earnings.payoutRequested')}`
                  : t('earnings.requestPayoutBtn')}
            </button>
            {!wiseRecipient && !wiseLoading && (
              <p style={{ color: 'var(--color-warning)', fontSize: '0.875rem', marginTop: '0.5rem', fontFamily: 'var(--font-sans)' }}>
                {t('earnings.addRecipientFirst')}
              </p>
            )}
            {withdrawError && (
              <p role="alert" style={{ color: 'var(--color-error)', fontSize: '0.875rem', marginTop: '0.5rem' }}>
                {withdrawError}
              </p>
            )}
          </form>
          <p className="withdraw-note">
            {t('earnings.payoutNote')}
          </p>
        </div>

        {/* ── Recent Payout Status ───────────────────────────────────────── */}
        {payoutStatuses.length > 0 && (
          <div style={{ marginTop: '1.5rem', padding: '1rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)' }}>
            <span className="kicker" style={{ marginBottom: '0.5rem', display: 'block' }}>// {t('earnings.payoutStatusKicker')}</span>
            <table className="payouts-table" aria-label={t('earnings.payoutStatusTable')}>
              <thead>
                <tr>
                  <th scope="col">{t('earnings.colRequested')}</th>
                  <th scope="col">{t('earnings.colAmount')}</th>
                  <th scope="col">{t('earnings.colStatus')}</th>
                </tr>
              </thead>
              <tbody>
                {payoutStatuses.slice(0, 5).map(p => (
                  <tr key={p.id}>
                    <td className="date-cell">{p.created_at ? p.created_at.slice(0, 10) : p.date ?? '—'}</td>
                    <td className="amount-cell positive">{formatAmount(p.amount ?? 0)}</td>
                    <td><span className={`status-badge ${p.status?.toLowerCase() ?? ''}`}>{p.status ?? '—'}</span></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="earnings-tabs" role="tablist" style={{ marginTop: '2rem' }} aria-label={t('earnings.tabsLabel')}>
          <button
            id="tab-transactions"
            role="tab"
            aria-selected={activeTab === 'transactions'}
            aria-controls="panel-transactions"
            className={`tab-btn ${activeTab === 'transactions' ? 'active' : ''}`}
            onClick={() => setActiveTab('transactions')}
          >
            {t('earnings.transactionHistory')}
          </button>
          <button
            id="tab-payouts"
            role="tab"
            aria-selected={activeTab === 'payouts'}
            aria-controls="panel-payouts"
            className={`tab-btn ${activeTab === 'payouts' ? 'active' : ''}`}
            onClick={() => setActiveTab('payouts')}
          >
            {t('earnings.payoutHistory')}
          </button>
        </div>

        <div id={`panel-${activeTab}`} className="tab-content" role="tabpanel" aria-labelledby={`tab-${activeTab}`}>
          {activeTab === 'transactions' ? (
            <div className="transactions-section">
              {loading ? (
                <div className="loading-state">
                  <span className="spinner large" aria-hidden="true" />
                  <span>{t('earnings.loadingTransactions')}</span>
                </div>
              ) : transactions.length === 0 ? (
                <p style={{ padding: '2rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  {t('earnings.noTransactions')}
                </p>
              ) : (
                <table className="transactions-table" aria-label={t('earnings.transactionsTableLabel')}>
                  <thead>
                    <tr>
                      <th scope="col">{t('earnings.colDate')}</th>
                      <th scope="col">{t('earnings.colDescription')}</th>
                      <th scope="col">{t('earnings.colAmount')}</th>
                      <th scope="col">{t('earnings.colBalance')}</th>
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
                  <span className="spinner large" aria-hidden="true" />
                  <span>{t('earnings.loadingPayouts')}</span>
                </div>
              ) : payouts.length === 0 ? (
                <p style={{ padding: '2rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  {t('earnings.noPayouts')}
                </p>
              ) : (
                <table className="payouts-table" aria-label={t('earnings.payoutsTableLabel')}>
                  <thead>
                    <tr>
                      <th scope="col">{t('earnings.colDate')}</th>
                      <th scope="col">{t('earnings.colAmount')}</th>
                      <th scope="col">{t('earnings.colMethod')}</th>
                      <th scope="col">{t('earnings.colStatus')}</th>
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
