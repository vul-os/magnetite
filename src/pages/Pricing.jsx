import PricingCard from '../components/PricingCard';
import './Pricing.css';

const SUBSCRIPTION_TIERS = [
  {
    id: 'free',
    name: 'Free',
    price: 0,
    priceDisplay: 'Free',
    features: [
      'Play any free Rust game',
      'Limited access to game catalog',
      'Basic matchmaking',
      'Community support',
    ],
    limitations: [
      'No premium games',
      'Limited play hours',
      'No priority support',
    ],
  },
  {
    id: 'basic',
    name: 'Basic',
    price: 4.99,
    priceDisplay: '$4.99',
    features: [
      'Access to all games',
      '10 hours playtime per month',
      'Standard matchmaking',
      'Email support',
    ],
    limitations: [
      'No unlimited hours',
      'No priority matchmaking',
    ],
  },
  {
    id: 'pro',
    name: 'Pro',
    price: 9.99,
    priceDisplay: '$9.99',
    features: [
      'Access to all games',
      '50 hours playtime per month',
      'Priority matchmaking',
      'Priority email support',
      'Early access to new Rust games',
    ],
    limitations: [],
    highlight: true,
  },
  {
    id: 'unlimited',
    name: 'Unlimited',
    price: 19.99,
    priceDisplay: '$19.99',
    features: [
      'Access to all games',
      'Unlimited playtime',
      'Priority matchmaking',
      '24/7 priority support',
      'Early access to new Rust games',
      'Exclusive tournaments',
    ],
    limitations: [],
  },
];

const COMPARISON_FEATURES = [
  { feature: 'Game Access',   free: 'Free games only', basic: 'All games',   pro: 'All games',   unlimited: 'All games' },
  { feature: 'Monthly Hours', free: '0 hours',         basic: '10 hours',    pro: '50 hours',    unlimited: 'Unlimited' },
  { feature: 'Matchmaking',   free: 'Standard',        basic: 'Standard',    pro: 'Priority',    unlimited: 'Priority' },
  { feature: 'Support',       free: 'Community',       basic: 'Email',       pro: 'Priority Email', unlimited: '24/7 Priority' },
  { feature: 'Early Access',  free: 'No',              basic: 'No',          pro: 'Yes',         unlimited: 'Yes' },
  { feature: 'Tournaments',   free: 'No',              basic: 'No',          pro: 'No',          unlimited: 'Yes' },
];

const faqs = [
  {
    q: 'How do subscription hours work?',
    a: 'Your monthly hours are consumed when playing premium games. Free games do not use your monthly allocation. Hours reset at the beginning of each billing cycle.',
  },
  {
    q: 'Can I upgrade or downgrade my plan?',
    a: 'Yes, you can change your subscription tier at any time. Upgrades take effect immediately, while downgrades apply at the start of your next billing cycle.',
  },
  {
    q: 'What happens if I exceed my monthly hours?',
    a: 'If you exceed your monthly hours, you can purchase additional hour packs or upgrade to a higher tier.',
  },
  {
    q: 'Do unused hours roll over?',
    a: 'No, unused monthly hours do not roll over. Your hour allocation resets each billing cycle.',
  },
  {
    q: 'Can I cancel my subscription anytime?',
    a: 'Yes, cancel at any time. You will retain access until the end of your current billing period.',
  },
  {
    q: 'What payment methods are accepted?',
    a: 'We accept USDC (on-chain) and Paystack (ZAR fiat on-ramp) for subscription payments.',
  },
];

export default function Pricing() {
  const user = JSON.parse(localStorage.getItem('user') || '{}');
  const currentPlan = user?.subscription?.tier || null;

  return (
    <div className="pricing-page">
      {/* ── Hero ─────────────────────────────────────────────────────────── */}
      <section className="pricing-hero bg-atmosphere" aria-labelledby="pricing-heading">
        <div className="container">
          <div className="reveal">
            <span className="kicker reveal-1">// TRANSPARENT PRICING</span>
            <h1 id="pricing-heading" className="pricing-hero-title reveal-2">
              Simple, honest pricing
            </h1>
            <p className="pricing-hero-subtitle reveal-3">
              Choose the plan that fits your gaming style. USDC or fiat — no hidden fees, no
              long-term contracts.
            </p>
          </div>
        </div>
      </section>

      {/* ── Cards ─────────────────────────────────────────────────────────── */}
      <section className="pricing-cards">
        <div className="container">
          <div className="pricing-grid">
            {SUBSCRIPTION_TIERS.map((tier) => (
              <PricingCard
                key={tier.id}
                type="subscription"
                title={tier.name}
                price={tier.priceDisplay}
                features={tier.features}
                cta={{
                  label:
                    currentPlan === tier.id
                      ? 'Current Plan'
                      : tier.price === 0
                      ? 'Get Started'
                      : 'Subscribe',
                  href:
                    currentPlan === tier.id ? '#' : `/subscribe/${tier.id}`,
                }}
                highlight={tier.highlight}
                isCurrentPlan={currentPlan === tier.id}
                limitations={tier.limitations}
              />
            ))}
          </div>
        </div>
      </section>

      {/* ── Comparison ────────────────────────────────────────────────────── */}
      <section className="pricing-comparison">
        <div className="container">
          <span className="kicker">// PLAN COMPARISON</span>
          <h2 className="pricing-section-title">Compare plans</h2>
          <div className="comparison-table-wrapper">
            <table className="comparison-table">
              <thead>
                <tr>
                  <th scope="col">Feature</th>
                  <th scope="col" className="comparison-accent">Free</th>
                  <th scope="col">Basic</th>
                  <th scope="col" className="comparison-accent">Pro</th>
                  <th scope="col">Unlimited</th>
                </tr>
              </thead>
              <tbody>
                {COMPARISON_FEATURES.map((row, i) => (
                  <tr key={i}>
                    <td>{row.feature}</td>
                    <td className="comparison-accent">{row.free}</td>
                    <td>{row.basic}</td>
                    <td className="comparison-accent">{row.pro}</td>
                    <td>{row.unlimited}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </section>

      {/* ── FAQ ───────────────────────────────────────────────────────────── */}
      <section className="pricing-faq">
        <div className="container">
          <span className="kicker">// FAQ</span>
          <h2 className="pricing-section-title">Frequently asked questions</h2>
          <div className="faq-grid">
            {faqs.map((faq, i) => (
              <div key={i} className="faq-item">
                <h3 className="faq-question">{faq.q}</h3>
                <p className="faq-answer">{faq.a}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── CTA ───────────────────────────────────────────────────────────── */}
      <section className="pricing-cta">
        <div className="container">
          <span className="kicker">// GET STARTED</span>
          <h2>Ready to start playing Rust games?</h2>
          <p>Create your free account and access the catalog today.</p>
          <div className="cta-buttons">
            <a href="/register" className="btn btn-primary btn-lg">Create Account</a>
            <a href="/marketplace" className="btn btn-secondary btn-lg">Browse Games</a>
          </div>
        </div>
      </section>
    </div>
  );
}
