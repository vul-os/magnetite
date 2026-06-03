import PricingCard from '../components/PricingCard';
import { useTranslation } from '../i18n/useTranslation';
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
    a: 'We accept credit and debit cards, and bank transfers via Paystack. All payments are in USD — no crypto required.',
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
              {t('store.pricingSubtitle')}
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
                      : t('store.subscribe'),
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
