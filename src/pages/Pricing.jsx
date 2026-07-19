import PricingCard from '../components/PricingCard';
import { useTranslation } from '../i18n/useTranslation';
import './Pricing.css';

/**
 * Tiers are **receipt-backed feature flags**, not billing plans (§3.6 PaymentRail).
 * You pay the operator's wallet in USDC from your own wallet; the checkout emits a
 * signed receipt and the node reads that receipt to unlock the tier for its period.
 * No card, no custody, no recurring mandate — when the receipt's period is over the
 * tier simply lapses unless you perform another checkout.
 */
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
    priceDisplay: '4.99 USDC',
    features: [
      'Access to all games',
      '10 hours playtime per 30-day receipt',
      'Standard matchmaking',
      'Community support',
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
    priceDisplay: '9.99 USDC',
    features: [
      'Access to all games',
      '50 hours playtime per 30-day receipt',
      'Priority matchmaking',
      'Priority support',
      'Early access to new Rust games',
    ],
    limitations: [],
    highlight: true,
  },
  {
    id: 'unlimited',
    name: 'Unlimited',
    price: 19.99,
    priceDisplay: '19.99 USDC',
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
  { feature: 'Price',         free: 'Free',            basic: '4.99 USDC',   pro: '9.99 USDC',   unlimited: '19.99 USDC' },
  { feature: 'Game Access',   free: 'Free games only', basic: 'All games',   pro: 'All games',   unlimited: 'All games' },
  { feature: 'Hours / receipt', free: '0 hours',       basic: '10 hours',    pro: '50 hours',    unlimited: 'Unlimited' },
  { feature: 'Matchmaking',   free: 'Standard',        basic: 'Standard',    pro: 'Priority',    unlimited: 'Priority' },
  { feature: 'Support',       free: 'Community',       basic: 'Community',   pro: 'Priority',    unlimited: '24/7 Priority' },
  { feature: 'Early Access',  free: 'No',              basic: 'No',          pro: 'Yes',         unlimited: 'Yes' },
  { feature: 'Tournaments',   free: 'No',              basic: 'No',          pro: 'No',          unlimited: 'Yes' },
];

const faqs = [
  {
    q: 'How do tier hours work?',
    a: 'Your hours are consumed when playing premium games. Free games do not use your allocation. Hours reset when you check out a new receipt for a fresh period.',
  },
  {
    q: 'Can I change tier?',
    a: 'Yes. A tier is just a signed receipt, so you can check out a different tier whenever you like. The new receipt takes effect as soon as the rail settles it — there is no proration to reconcile because nothing is billed in arrears.',
  },
  {
    q: 'What happens if I run out of hours?',
    a: 'Buy an hour pack or check out a higher tier. Both are ordinary wallet checkouts that produce their own receipt.',
  },
  {
    q: 'Do unused hours roll over?',
    a: 'No. Your allocation is scoped to the receipt that granted it and resets with the next one.',
  },
  {
    q: 'How do I cancel?',
    a: 'There is nothing to cancel. Nobody holds a mandate against your wallet, so if you do not perform another checkout the tier simply lapses at the end of its period and you drop back to Free.',
  },
  {
    q: 'How do payments work?',
    a: 'Non-custodially, in USDC. You approve one transfer from your own wallet to the operator’s wallet and the rail returns a signed receipt; the node reads that receipt to unlock your tier. We never take custody of funds, never see a card, and store no bank details.',
  },
  {
    q: 'What is the protocol fee?',
    a: 'Checkout carries a protocol fee expressed in basis points, currently 0 bps by default — the developer or operator receives the full subtotal. Any fee is applied by the rail at settlement and is itemised on the receipt.',
  },
];

export default function Pricing() {
  const { t } = useTranslation();
  const user = JSON.parse(localStorage.getItem('user') || '{}');
  const currentPlan = user?.subscription?.tier || null;

  return (
    <div className="pricing-page">
      {/* ── Hero ─────────────────────────────────────────────────────────── */}
      <section className="pricing-hero bg-atmosphere" aria-labelledby="pricing-heading">
        <div className="container">
          <div className="reveal">
            <span className="kicker reveal-1">{t('store.pricingKicker')}</span>
            <h1 id="pricing-heading" className="pricing-hero-title reveal-2">
              {t('store.pricingTitle')}
            </h1>
            <p className="pricing-hero-subtitle reveal-3">
              Choose the tier that fits your gaming style. Prices are in USDC and paid straight
              from your own wallet — one transfer, one signed receipt, no custody and no
              recurring mandate.
            </p>
          </div>
        </div>
      </section>

      {/* ── Cards ─────────────────────────────────────────────────────────── */}
      <section className="pricing-cards" aria-label="Subscription plans">
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
                      ? t('store.currentPlan')
                      : tier.price === 0
                      ? t('store.getStarted')
                      : 'Pay with wallet',
                  href:
                    currentPlan === tier.id ? '#' : `/subscribe/${tier.id}`,
                }}
                highlight={tier.highlight}
                isCurrentPlan={currentPlan === tier.id}
                limitations={tier.limitations}
              />
            ))}
          </div>
          <p
            className="pricing-rail-note"
            style={{
              maxWidth: '72ch',
              margin: '2rem auto 0',
              textAlign: 'center',
              fontSize: 'var(--text-sm)',
              lineHeight: 1.6,
              color: 'var(--color-text-secondary)',
            }}
          >
            Checkout is non-custodial: USDC moves from your wallet to the operator&rsquo;s wallet in
            one atomic transfer and the rail hands back a signed receipt. That receipt is what
            unlocks the tier — there is no card on file, no stored balance and nothing to cancel.
            When a receipt&rsquo;s period ends the tier simply lapses.
          </p>
        </div>
      </section>

      {/* ── Comparison ────────────────────────────────────────────────────── */}
      <section className="pricing-comparison" aria-labelledby="comparison-heading">
        <div className="container">
          <span className="kicker">{t('store.planComparisonKicker')}</span>
          <h2 id="comparison-heading" className="pricing-section-title">{t('store.planComparison')}</h2>
          <div className="comparison-table-wrapper" role="region" aria-label={t('store.planComparison')}>
            <table className="comparison-table">
              <caption className="visually-hidden">{t('store.planComparison')}</caption>
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
      <section className="pricing-faq" aria-labelledby="faq-heading">
        <div className="container">
          <span className="kicker">{t('store.faqKicker')}</span>
          <h2 id="faq-heading" className="pricing-section-title">{t('store.faqTitle')}</h2>
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
      <section className="pricing-cta" aria-labelledby="pricing-cta-heading">
        <div className="container">
          <span className="kicker">{t('store.ctaKicker')}</span>
          <h2 id="pricing-cta-heading">{t('store.ctaTitle')}</h2>
          <p>{t('store.ctaBody')}</p>
          <div className="cta-buttons">
            <a href="/register" className="btn btn-primary btn-lg">{t('store.ctaCreateAccount')}</a>
            <a href="/marketplace" className="btn btn-secondary btn-lg">{t('store.ctaBrowse')}</a>
          </div>
        </div>
      </section>
    </div>
  );
}
