import { useState } from 'react';
import Layout from '../components/Layout';
import SubscriptionCard from '../components/SubscriptionCard';
import SubscriptionBadge from '../components/SubscriptionBadge';
import UsageMeter from '../components/UsageMeter';
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

const MOCK_PAYMENT_HISTORY = [
  { id: 1, date: '2026-05-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
  { id: 2, date: '2026-04-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
  { id: 3, date: '2026-03-01', amount: 24.99, description: 'Pro Plan - Monthly', status: 'completed' },
  { id: 4, date: '2026-02-01', amount: 9.99, description: 'Basic Plan - Monthly', status: 'completed' },
  { id: 5, date: '2026-01-01', amount: 9.99, description: 'Basic Plan - Monthly', status: 'completed' },
];

export default function Subscription() {
  const [currentSubscription] = useState({
    tier: 'pro',
    renewalDate: '2026-06-01',
    hoursUsed: 67,
    hoursTotal: 100,
  });

  const handleSubscribe = (tier) => {
    console.log('Subscribe to:', tier.name);
  };

  const handleCancelSubscription = () => {
    if (window.confirm('Are you sure you want to cancel your subscription? You will lose access to Pro features at the end of your billing period.')) {
      console.log('Cancel subscription');
    }
  };

  const formatDate = (dateStr) => {
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
          <span className="kicker">// BILLING & PLANS</span>
          <h1>Subscription</h1>
          <p>Manage your subscription plan and billing for Rust game access</p>
        </header>

        <div className="subscription-content">
          <div className="subscription-main">
            <section className="current-plan-section">
              <h2>Current Plan</h2>
              <div className="current-plan-card">
                <div className="current-plan-header">
                  <SubscriptionBadge tier={currentSubscription.tier} size="lg" />
                  <span className="current-plan-label">Active</span>
                </div>
                <div className="current-plan-details">
                  <div className="detail-row">
                    <span className="detail-label">Renews on</span>
                    <span className="detail-value">{formatDate(currentSubscription.renewalDate)}</span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">Billing cycle</span>
                    <span className="detail-value">Monthly</span>
                  </div>
                </div>
                <UsageMeter
                  used={currentSubscription.hoursUsed}
                  limit={currentSubscription.hoursTotal}
                />
              </div>
            </section>

            <section className="plans-section">
              <h2>Available Plans</h2>
              <div className="plans-grid">
                {TIERS.map((tier) => (
                  <SubscriptionCard
                    key={tier.id}
                    tier={tier}
                    isCurrent={tier.name.toLowerCase() === currentSubscription.tier}
                    onSubscribe={handleSubscribe}
                  />
                ))}
              </div>
            </section>
          </div>

          <aside className="subscription-sidebar">
            <section className="payment-history-section">
              <h3>Payment History</h3>
              <div className="payment-list">
                {MOCK_PAYMENT_HISTORY.map((payment) => (
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
              <button className="view-all-btn">View All Transactions</button>
            </section>

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
                >
                  Cancel Subscription
                </button>
              </div>
            </section>
          </aside>
        </div>
      </div>
    </Layout>
  );
}
