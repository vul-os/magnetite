import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import SubscriptionCard from '../components/SubscriptionCard';
import SubscriptionBadge from '../components/SubscriptionBadge';
import UsageMeter from '../components/UsageMeter';
import { api } from '../api/client';
import './Subscription.css';

const TIERS = [
  {
    id: 'free',
    name: 'Free',
    price: '$0',
    period: 'month',
    features: [
      '10 hours per month',
      'Basic Rust game access',
      'Community support',
      'Standard matchmaking',
    ],
  },
  {
    id: 'basic',
    name: 'Basic',
    price: '$9.99',
    period: 'month',
    features: [
      '50 hours per month',
      'Extended game library',
      'Priority support',
      'Faster matchmaking',
      'Ad-free experience',
    ],
  },
  {
    id: 'pro',
    name: 'Pro',
    price: '$24.99',
    period: 'month',
    recommended: true,
    features: [
      '100 hours per month',
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
    price: '$49.99',
    period: 'month',
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
const MOCK_PAYMENT_HISTORY = import.meta.env.VITE_USE_MOCKS
  ? [
      { id: 1, date: '2026-05-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
      { id: 2, date: '2026-04-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
      { id: 3, date: '2026-03-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
    ]
  : null;

const MOCK_SUBSCRIPTION = import.meta.env.VITE_USE_MOCKS
  ? { tier: 'pro', renewalDate: '2026-06-01', hoursUsed: 67, hoursTotal: 100 }
  : null;

export default function Subscription() {
  const [currentSubscription, setCurrentSubscription] = useState(MOCK_SUBSCRIPTION);
  const [paymentHistory, _setPaymentHistory]          = useState(MOCK_PAYMENT_HISTORY ?? []);
  const [loading, setLoading]       = useState(!MOCK_SUBSCRIPTION);
  const [error, setError]           = useState(null);
  const [subscribing, setSubscribing]   = useState(false);
  const [cancelling, setCancelling]     = useState(false);
  const [actionError, setActionError]   = useState(null);
  const [actionSuccess, setActionSuccess] = useState(null);

  const fetchSubscription = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) return;
    setLoading(true);
    setError(null);
    try {
      const [subData, usageData] = await Promise.allSettled([
        api.subscriptions.current(),
        api.subscriptions.usage(),
      ]);

      if (subData.status === 'fulfilled' && subData.value) {
        const d = subData.value;
        const usage = usageData.status === 'fulfilled' ? usageData.value : null;
        setCurrentSubscription({
          tier:        d.tier ?? d.plan_id ?? 'free',
          renewalDate: d.renewal_date ?? d.renews_at ?? null,
          hoursUsed:   usage?.hours_used  ?? d.hours_used  ?? 0,
          hoursTotal:  usage?.hours_total ?? d.hours_limit ?? 0,
        });
      }
    } catch (err) {
      setError(err.message || 'Failed to load subscription');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchSubscription(); }, [fetchSubscription]);

  const handleSubscribe = async (tier) => {
    if (tier.id === 'free') return;
    setSubscribing(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      await api.subscriptions.create({ plan_id: tier.id, tier: tier.id });
      setActionSuccess(`Subscribed to ${tier.name}!`);
      setTimeout(() => setActionSuccess(null), 3000);
      await fetchSubscription();
    } catch (err) {
      setActionError(err.message || 'Failed to subscribe');
    } finally {
      setSubscribing(false);
    }
  };

  const handleCancelSubscription = async () => {
    if (!window.confirm('Are you sure you want to cancel your subscription? You will lose access to Pro features at the end of your billing period.')) {
      return;
    }
    setCancelling(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      await api.subscriptions.cancel();
      setActionSuccess('Subscription cancelled. Access continues until the end of your billing period.');
      setTimeout(() => setActionSuccess(null), 4000);
      await fetchSubscription();
    } catch (err) {
      setActionError(err.message || 'Failed to cancel subscription');
    } finally {
      setCancelling(false);
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

  return (
    <Layout>
      <div className="subscription-page">
        <header className="subscription-header">
          <span className="kicker">// BILLING &amp; PLANS</span>
          <h1>Subscription</h1>
          <p>Manage your subscription plan and billing for Rust game access</p>
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
                <h2>Current Plan</h2>
                <div className="current-plan-card">
                  <div className="current-plan-header">
                    <SubscriptionBadge tier={currentSubscription.tier} size="lg" />
                    <span className="current-plan-label">Active</span>
                  </div>
                  <div className="current-plan-details">
                    {currentSubscription.renewalDate && (
                      <div className="detail-row">
                        <span className="detail-label">Renews on</span>
                        <span className="detail-value">{formatDate(currentSubscription.renewalDate)}</span>
                      </div>
                    )}
                    <div className="detail-row">
                      <span className="detail-label">Billing cycle</span>
                      <span className="detail-value">Monthly</span>
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

            <section className="plans-section">
              <h2>Available Plans</h2>
              <div className="plans-grid">
                {TIERS.map((tier) => (
                  <SubscriptionCard
                    key={tier.id}
                    tier={tier}
                    isCurrent={currentSubscription
                      ? tier.name.toLowerCase() === currentSubscription.tier
                      : tier.id === 'free'}
                    onSubscribe={subscribing ? () => {} : handleSubscribe}
                  />
                ))}
              </div>
            </section>
          </div>

          <aside className="subscription-sidebar">
            <section className="payment-history-section">
              <h3>Payment History</h3>
              {paymentHistory.length === 0 ? (
                <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>
                  No payment history
                </p>
              ) : (
                <div className="payment-list">
                  {paymentHistory.map((payment) => (
                    <div key={payment.id} className="payment-item">
                      <div className="payment-info">
                        <span className="payment-description">{payment.description}</span>
                        <span className="payment-date">{formatDate(payment.date)}</span>
                      </div>
                      <div className="payment-right">
                        <span className="payment-amount">${payment.amount.toFixed(2)}</span>
                        <span className={`payment-status ${payment.status}`}>{payment.status}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>

            {currentSubscription && currentSubscription.tier !== 'free' && (
              <section className="danger-zone-section">
                <h3>Danger Zone</h3>
                <div className="danger-card">
                  <div className="danger-info">
                    <span className="danger-title">Cancel Subscription</span>
                    <span className="danger-desc">
                      You will lose access to Pro features at the end of your billing period.
                    </span>
                  </div>
                  <button
                    className="btn btn-danger"
                    onClick={handleCancelSubscription}
                    disabled={cancelling}
                  >
                    {cancelling ? 'Cancelling…' : 'Cancel Subscription'}
                  </button>
                </div>
              </section>
            )}
          </aside>
        </div>
      </div>
    </Layout>
  );
}
