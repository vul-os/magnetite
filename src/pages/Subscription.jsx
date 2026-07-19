import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import SubscriptionCard from '../components/SubscriptionCard';
import SubscriptionBadge from '../components/SubscriptionBadge';
import UsageMeter from '../components/UsageMeter';
import { api } from '../api/client';
import './Subscription.css';

/**
 * Tiers are receipt-backed feature flags (§3.6 PaymentRail), not billing plans.
 * A checkout moves USDC from the subscriber's wallet to the operator's wallet in one
 * atomic transfer and returns a signed receipt; the node reads that receipt to unlock
 * the tier for its period. There is no card, no custodial balance and no recurring
 * mandate — "renewal" is simply a new checkout, and doing nothing lets the tier lapse.
 */
const TIERS = [
  {
    id: 'free',
    name: 'Free',
    price: 'Free',
    period: null,
    features: [
      '10 hours per period',
      'Basic Rust game access',
      'Community support',
      'Standard matchmaking',
    ],
  },
  {
    id: 'basic',
    name: 'Basic',
    price: '9.99 USDC',
    period: '30 days',
    features: [
      '50 hours per period',
      'Extended game library',
      'Priority support',
      'Faster matchmaking',
      'Ad-free experience',
    ],
  },
  {
    id: 'pro',
    name: 'Pro',
    price: '24.99 USDC',
    period: '30 days',
    recommended: true,
    features: [
      '100 hours per period',
      'Full Rust game access',
      '24/7 priority support',
      'Instant matchmaking',
      'Exclusive tournaments',
      'Advanced analytics',
    ],
  },
  {
    id: 'unlimited',
    name: 'Unlimited',
    price: '49.99 USDC',
    period: '30 days',
    features: [
      'Unlimited hours',
      'Full Rust game access',
      'VIP support',
      'Instant matchmaking',
      'Exclusive tournaments',
      'Advanced analytics',
      'Cloud save sync',
      'Early access features',
    ],
  },
];

/* Mock data — only used when VITE_USE_MOCKS=true */
const MOCK_RECEIPTS = import.meta.env.VITE_USE_MOCKS
  ? [
      { id: 'rcpt_01HZM6B2C8D4E1F7', date: '2026-05-01', total: 24.99, protocol_fee: 0, description: 'Pro tier — 30 days', rail_pubkey: 'ed25519:0c94ae6217fb3d80', status: 'settled' },
      { id: 'rcpt_01HZ8P4K7T2R9WNX', date: '2026-04-01', total: 24.99, protocol_fee: 0, description: 'Pro tier — 30 days', rail_pubkey: 'ed25519:0c94ae6217fb3d80', status: 'settled' },
      { id: 'rcpt_01HZ1D5J3M6V8QYB', date: '2026-03-01', total: 24.99, protocol_fee: 0, description: 'Pro tier — 30 days', rail_pubkey: 'ed25519:0c94ae6217fb3d80', status: 'settled' },
    ]
  : null;

const MOCK_SUBSCRIPTION = import.meta.env.VITE_USE_MOCKS
  ? { tier: 'pro', lapsesAt: '2026-06-01', hoursUsed: 67, hoursTotal: 100, receiptId: 'rcpt_01HZM6B2C8D4E1F7' }
  : null;

const shortKey = (v) => {
  if (!v) return '—';
  const raw = String(v).replace(/^[a-z0-9]+:/i, '');
  return raw.length <= 14 ? raw : `${raw.slice(0, 8)}…${raw.slice(-4)}`;
};

const usdc = (amount) =>
  `${Number(amount ?? 0).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDC`;

export default function Subscription() {
  const [currentSubscription, setCurrentSubscription] = useState(MOCK_SUBSCRIPTION);
  const [receipts, setReceipts]         = useState(MOCK_RECEIPTS ?? []);
  const [loading, setLoading]           = useState(!MOCK_SUBSCRIPTION);
  const [error, setError]               = useState(null);
  const [checkingOut, setCheckingOut]   = useState(false);
  const [actionError, setActionError]   = useState(null);
  const [actionSuccess, setActionSuccess] = useState(null);
  // Checkout flow: confirm the wallet transfer for a chosen tier
  const [checkoutTarget, setCheckoutTarget] = useState(null);
  const [receiptRef, setReceiptRef]         = useState('');

  const fetchSubscription = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) return;
    setLoading(true);
    setError(null);
    try {
      const [subData, usageData, receiptData] = await Promise.allSettled([
        api.subscriptions.current(),
        api.subscriptions.usage(),
        api.subscriptions.receipts?.() ?? Promise.reject(new Error('unavailable')),
      ]);

      if (subData.status === 'fulfilled' && subData.value) {
        const d = subData.value;
        const usage = usageData.status === 'fulfilled' ? usageData.value : null;
        setCurrentSubscription({
          tier:       d.tier ?? d.plan_id ?? 'free',
          lapsesAt:   d.lapses_at ?? d.current_period_end ?? d.renewal_date ?? null,
          hoursUsed:  usage?.hours_used  ?? d.hours_used  ?? 0,
          hoursTotal: usage?.hours_total ?? d.hours_limit ?? 0,
          receiptId:  d.receipt_id ?? null,
          railPubkey: d.rail_pubkey ?? null,
        });
      }

      if (receiptData.status === 'fulfilled') {
        const r = receiptData.value?.data ?? receiptData.value;
        setReceipts(Array.isArray(r?.receipts) ? r.receipts : Array.isArray(r) ? r : []);
      }
    } catch (err) {
      setError(err.message || 'Failed to load subscription');
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch the subscription from the API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchSubscription(); }, [fetchSubscription]);

  const openCheckout = (tier) => {
    if (tier.id === 'free') return;
    if (currentSubscription && tier.name.toLowerCase() === currentSubscription.tier) return;
    setCheckoutTarget(tier);
    setReceiptRef('');
    setActionError(null);
    setActionSuccess(null);
  };

  const handleConfirmCheckout = async () => {
    if (!checkoutTarget) return;
    setCheckingOut(true);
    setActionError(null);
    try {
      const res = currentSubscription
        ? await api.subscriptions.upgrade(checkoutTarget.id, receiptRef || undefined)
        : await api.subscriptions.create({ plan_id: checkoutTarget.id, tier: checkoutTarget.id, currency: 'usdc', receipt_id: receiptRef || undefined });
      const receipt = res?.receipt ?? res ?? {};
      const rid = receipt.receipt_id ?? receipt.id ?? receiptRef;
      setActionSuccess(
        rid
          ? `${checkoutTarget.name} unlocked — receipt ${shortKey(rid)}`
          : `${checkoutTarget.name} unlocked.`
      );
      setCheckoutTarget(null);
      setReceiptRef('');
      setTimeout(() => setActionSuccess(null), 5000);
      await fetchSubscription();
    } catch (err) {
      setActionError(err.message || 'Checkout failed. If you already paid, paste the receipt ID above and try again.');
    } finally {
      setCheckingOut(false);
    }
  };

  const formatDate = (dateStr) => {
    if (!dateStr) return 'N/A';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    });
  };

  const currentTierId = currentSubscription?.tier?.toLowerCase() ?? 'free';
  const isCurrentTier = (tier) =>
    tier.name.toLowerCase() === currentTierId || tier.id === currentTierId;

  return (
    <Layout>
      <div className="subscription-page">
        <header className="subscription-header">
          <span className="kicker">// TIERS &amp; RECEIPTS</span>
          <h1>Subscription</h1>
          <p>
            Tiers are unlocked by a signed receipt from a wallet checkout in USDC.
            No card, no stored balance, no recurring mandate.
          </p>
        </header>

        {actionError && (
          <div className="auth-error" role="alert" style={{ marginBottom: '1rem', maxWidth: 600, margin: '0 auto 1rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {actionError}
          </div>
        )}

        {actionSuccess && (
          <div className="auth-success" role="status" style={{ marginBottom: '1rem', maxWidth: 600, margin: '0 auto 1rem' }}>
            <span className="auth-success-icon" aria-hidden="true">✓</span>
            {actionSuccess}
          </div>
        )}

        <div className="subscription-content">
          <div className="subscription-main">
            {loading ? (
              <section className="current-plan-section">
                <div className="current-plan-card" aria-busy="true" style={{ textAlign: 'center', padding: '2rem', color: 'var(--color-text-muted)' }}>
                  <span className="spinner" aria-hidden="true" /> Loading subscription&hellip;
                </div>
              </section>
            ) : error ? (
              <section className="current-plan-section">
                <div className="current-plan-card" role="alert" style={{ color: 'var(--color-error)', padding: '1rem' }}>
                  {error}
                  <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={fetchSubscription}>
                    Retry
                  </button>
                </div>
              </section>
            ) : currentSubscription ? (
              <section className="current-plan-section">
                <h2>Current Tier</h2>
                <div className="current-plan-card">
                  <div className="current-plan-header">
                    <SubscriptionBadge tier={currentSubscription.tier} size="lg" />
                    <span className="current-plan-label">Active</span>
                  </div>
                  <div className="current-plan-details">
                    {currentSubscription.lapsesAt && (
                      <div className="detail-row">
                        <span className="detail-label">Lapses on</span>
                        <span className="detail-value">{formatDate(currentSubscription.lapsesAt)}</span>
                      </div>
                    )}
                    <div className="detail-row">
                      <span className="detail-label">Unlocked by receipt</span>
                      <span className="detail-value" style={{ fontFamily: 'var(--font-mono)' }} title={currentSubscription.receiptId ?? ''}>
                        {shortKey(currentSubscription.receiptId)}
                      </span>
                    </div>
                    <div className="detail-row">
                      <span className="detail-label">Rail</span>
                      <span className="detail-value" style={{ fontFamily: 'var(--font-mono)' }}>USDC · non-custodial</span>
                    </div>
                  </div>
                  {currentSubscription.hoursTotal > 0 && (
                    <UsageMeter
                      used={currentSubscription.hoursUsed}
                      limit={currentSubscription.hoursTotal}
                    />
                  )}
                </div>
              </section>
            ) : null}

            {/* Checkout confirmation panel */}
            {checkoutTarget && (
              <section style={{ marginBottom: '1.5rem' }}>
                <div style={{ padding: '1.25rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-accent)', borderRadius: 'var(--radius)', maxWidth: 520 }}>
                  <h3 style={{ margin: '0 0 0.75rem', fontSize: 'var(--text-base)' }}>
                    Check out <strong>{checkoutTarget.name}</strong> — {checkoutTarget.price}
                  </h3>
                  <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', marginBottom: '0.75rem', lineHeight: 1.6 }}>
                    Your wallet will transfer {checkoutTarget.price} to the operator&rsquo;s wallet in a
                    single atomic payment. The rail returns a signed receipt and the tier unlocks the
                    moment it settles. Nothing recurs — when the period ends the tier lapses unless
                    you check out again. Protocol fee: 0 bps.
                  </p>
                  <input
                    className="input"
                    type="text"
                    placeholder="Receipt ID (optional — only if you already paid)"
                    value={receiptRef}
                    onChange={e => setReceiptRef(e.target.value)}
                    aria-label="Existing receipt ID"
                    style={{ marginBottom: '0.75rem', width: '100%' }}
                  />
                  <div style={{ display: 'flex', gap: '0.5rem' }}>
                    <button
                      className="btn btn-primary"
                      onClick={handleConfirmCheckout}
                      disabled={checkingOut}
                    >
                      {checkingOut ? 'Settling…' : `Pay with wallet — ${checkoutTarget.price}`}
                    </button>
                    <button
                      className="btn btn-secondary"
                      onClick={() => { setCheckoutTarget(null); setActionError(null); }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </section>
            )}

            <section className="plans-section">
              <h2>Available Tiers</h2>
              <div className="plans-grid">
                {TIERS.map((tier) => (
                  <SubscriptionCard
                    key={tier.id}
                    tier={tier}
                    isCurrent={isCurrentTier(tier)}
                    onSubscribe={openCheckout}
                  />
                ))}
              </div>
            </section>
          </div>

          <aside className="subscription-sidebar">
            <section className="payment-history-section">
              <h3>Receipts</h3>
              {receipts.length === 0 ? (
                <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>
                  No receipts yet
                </p>
              ) : (
                <div className="payment-list">
                  {receipts.map((r) => (
                    <div key={r.id} className="payment-item">
                      <div className="payment-info">
                        <span className="payment-description">{r.description ?? 'Tier checkout'}</span>
                        <span className="payment-date">{formatDate(r.date ?? r.settled_at)}</span>
                        <span className="payment-date" style={{ fontFamily: 'var(--font-mono)' }} title={r.id}>
                          {shortKey(r.id)} · rail {shortKey(r.rail_pubkey)}
                        </span>
                      </div>
                      <div className="payment-right">
                        <span className="payment-amount">{usdc(r.total ?? r.amount)}</span>
                        <span className={`payment-status ${r.status ?? 'settled'}`}>{r.status ?? 'settled'}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section style={{ marginTop: '1rem', padding: '1rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)' }}>
              <h3 style={{ margin: '0 0 0.5rem', fontSize: 'var(--text-sm)' }}>Nothing to cancel</h3>
              <p style={{ color: 'var(--color-text-secondary)', fontSize: 'var(--text-sm)', margin: 0, fontFamily: 'var(--font-sans)', lineHeight: 1.6 }}>
                No one holds a mandate against your wallet. Your tier is valid for the period its
                receipt paid for and then lapses back to Free on its own. To keep it, perform a new
                checkout before it expires.
              </p>
            </section>
          </aside>
        </div>
      </div>
    </Layout>
  );
}
