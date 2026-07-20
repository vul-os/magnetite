import { useTranslation } from '../i18n/useTranslation';
import './Pricing.css';

/**
 * Tiers are **receipt-backed feature flags**, not billing plans (§3.6 PaymentRail).
 * You pay the operator's wallet in USDC from your own wallet; the checkout emits a
 * signed receipt and the node reads that receipt to unlock the tier for its period.
 * No card, no custody, no recurring mandate — when the receipt's period is over the
 * tier simply lapses unless you perform another checkout.
 *
 * ───────────────────────────────────────────────────────────────────────────────
 * ⚠ KNOWN DATA DISCREPANCY — UNRESOLVED FOUNDER DECISION. DO NOT "FIX" BY GUESSING.
 *
 * The figures below are HARDCODED in this file. They are NOT API-derived, and they
 * CONTRADICT the tier table hardcoded in `src/pages/Subscription.jsx`:
 *
 *              this page (Pricing.jsx)        src/pages/Subscription.jsx
 *   Free       Free                           Free       (10 hours per period)
 *   Basic       4.99 USDC / 10 hours          Basic       9.99 USDC / 50 hours
 *   Pro         9.99 USDC / 50 hours          Pro        24.99 USDC / 100 hours
 *   Unlimited  19.99 USDC / unlimited         Unlimited  49.99 USDC / unlimited
 *
 * The same plan therefore shows a different price and a different hour allocation
 * depending on which route the user lands on. Which set is canonical is an OPEN
 * FOUNDER DECISION; it has not been made, so neither file may be silently aligned
 * to the other. The real fix is a single server-owned source of truth that both
 * pages read. Until that decision lands, these numbers are preserved as-is and
 * this comment is the record of the conflict.
 * ───────────────────────────────────────────────────────────────────────────────
 */
const SUBSCRIPTION_TIERS = [
  {
    id: 'free',
    name: 'Free',
    price: 0,
    priceDisplay: 'Free',
    period: null,
    summary: 'Free games, standard matchmaking, no receipt required.',
  },
  {
    id: 'basic',
    name: 'Basic',
    price: 4.99,
    priceDisplay: '4.99 USDC',
    period: 'per 30-day receipt',
    summary: 'The whole catalog, metered hours.',
  },
  {
    id: 'pro',
    name: 'Pro',
    price: 9.99,
    priceDisplay: '9.99 USDC',
    period: 'per 30-day receipt',
    summary: 'More hours, priority queue and support.',
    highlight: true,
  },
  {
    id: 'unlimited',
    name: 'Unlimited',
    price: 19.99,
    priceDisplay: '19.99 USDC',
    period: 'per 30-day receipt',
    summary: 'No hour ceiling, plus tournaments.',
  },
];

/**
 * The tier matrix. Grouped so the reader can see, at a glance, which lines are
 * money, which are entitlement, and which are service level.
 * `mono: true` marks a row whose values are figures a user would compare or
 * verify — those are set in the mono face per the typography contract.
 */
const MATRIX_GROUPS = [
  {
    id: 'terms',
    label: 'Terms',
    rows: [
      { id: 'price', label: 'Price', mono: true, values: { free: 'Free', basic: '4.99 USDC', pro: '9.99 USDC', unlimited: '19.99 USDC' } },
      { id: 'period', label: 'Billing period', mono: true, values: { free: '—', basic: '30 days', pro: '30 days', unlimited: '30 days' } },
      { id: 'renewal', label: 'Auto-renewal', values: { free: 'None', basic: 'None', pro: 'None', unlimited: 'None' } },
    ],
  },
  {
    id: 'entitlement',
    label: 'Entitlement',
    rows: [
      { id: 'access', label: 'Game access', values: { free: 'Free games only', basic: 'All games', pro: 'All games', unlimited: 'All games' } },
      { id: 'hours', label: 'Hours per receipt', mono: true, values: { free: '0', basic: '10', pro: '50', unlimited: 'Unlimited' } },
      { id: 'rollover', label: 'Unused hours roll over', values: { free: 'No', basic: 'No', pro: 'No', unlimited: 'Not applicable' } },
      { id: 'early', label: 'Early access builds', values: { free: 'No', basic: 'No', pro: 'Yes', unlimited: 'Yes' } },
      { id: 'tournaments', label: 'Exclusive tournaments', values: { free: 'No', basic: 'No', pro: 'No', unlimited: 'Yes' } },
    ],
  },
  {
    id: 'service',
    label: 'Service level',
    rows: [
      { id: 'matchmaking', label: 'Matchmaking', values: { free: 'Standard', basic: 'Standard', pro: 'Priority', unlimited: 'Priority' } },
      { id: 'support', label: 'Support', values: { free: 'Community', basic: 'Community', pro: 'Priority', unlimited: '24/7 priority' } },
    ],
  },
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
      <section className="pricing-hero" aria-labelledby="pricing-heading">
        <div className="container pricing-hero-inner">
          <span className="kicker">{t('store.pricingKicker')}</span>
          <h1 id="pricing-heading" className="pricing-hero-title">
            {t('store.pricingTitle')}
          </h1>
          <p className="pricing-lede">
            A tier is a signed receipt, not a subscription. You approve one USDC transfer from
            your own wallet, the rail hands back a receipt, and the node reads that receipt to
            unlock the tier for its period. <span className="display-em">Nobody holds a mandate
            against your wallet.</span>
          </p>
        </div>
      </section>

      {/* ── The honest boundary ───────────────────────────────────────────── */}
      <section className="pricing-boundary" aria-labelledby="boundary-heading">
        <div className="container">
          <div className="panel pricing-boundary-panel">
            <div className="edge-boundary pricing-boundary-body">
              <p className="m-sm pricing-boundary-label">Where verification stops</p>
              <h2 id="boundary-heading" className="pricing-boundary-title">
                The payment rail is external
              </h2>
              <p className="pricing-boundary-text">
                Everything upstream of checkout is verifiable on your own machine: deterministic
                simulation, replay-checked matches, hash-addressed builds, keypair identity.
                Settlement is not. The transfer clears on a third-party chain and the receipt is
                signed by a wallet we do not control — so this one line is marked as a boundary
                rather than dressed up as part of the core. What we can state exactly is what we
                never touch: no card, no custody of funds, no stored bank details, no mandate.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* ── Tier matrix ───────────────────────────────────────────────────── */}
      <section className="pricing-matrix" aria-labelledby="matrix-heading">
        <div className="container">
          <span className="kicker">{t('store.planComparisonKicker')}</span>
          <h2 id="matrix-heading" className="pricing-section-title">{t('store.planComparison')}</h2>

          <div
            className="table-wrap pricing-table-wrap"
            role="region"
            aria-labelledby="matrix-heading"
            tabIndex={0}
          >
            <table className="data pricing-table">
              <caption className="visually-hidden">
                Exact terms for every tier. Prices are hardcoded on this page and are not
                API-derived.
              </caption>
              <thead>
                <tr>
                  <th scope="col" className="pricing-th-attr">Term</th>
                  {SUBSCRIPTION_TIERS.map((tier) => (
                    <th
                      key={tier.id}
                      scope="col"
                      className={currentPlan === tier.id ? 'pricing-th-tier is-current' : 'pricing-th-tier'}
                    >
                      <span className="pricing-th-name">{tier.name}</span>
                      {currentPlan === tier.id && (
                        <span className="st st-live pricing-th-current">{t('store.currentPlan')}</span>
                      )}
                    </th>
                  ))}
                </tr>
              </thead>

              {MATRIX_GROUPS.map((group) => (
                <tbody key={group.id} className="pricing-tgroup">
                  <tr className="pricing-group-row">
                    <th scope="colgroup" colSpan={SUBSCRIPTION_TIERS.length + 1} className="m-sm pricing-group-label">
                      {group.label}
                    </th>
                  </tr>
                  {group.rows.map((row) => (
                    <tr key={row.id}>
                      <th scope="row" className="pricing-row-label">{row.label}</th>
                      {SUBSCRIPTION_TIERS.map((tier) => (
                        <td
                          key={tier.id}
                          className={[
                            row.mono ? 'pricing-cell-num' : 'pricing-cell',
                            currentPlan === tier.id ? 'is-current' : '',
                          ].join(' ').trim()}
                        >
                          {row.values[tier.id]}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              ))}
            </table>
          </div>

          <p className="pricing-table-note">
            Figures on this page are hardcoded in the client and are not read from the node.
            Your node&rsquo;s receipt is the authority on what you are actually entitled to.
          </p>

          {/* Actions live outside the table so every control keeps a plain,
              unambiguous accessible name at any viewport width. */}
          <ul className="pricing-actions" aria-label="Check out a tier">
            {SUBSCRIPTION_TIERS.map((tier) => {
              const isCurrent = currentPlan === tier.id;
              return (
                <li
                  key={tier.id}
                  className={`pricing-action ${tier.highlight ? 'edge-field' : 'edge-none'}`}
                >
                  <p className="m-sm pricing-action-name">{tier.name}</p>
                  <p className="pricing-action-price font-mono">
                    {tier.priceDisplay}
                    {tier.period && <span className="pricing-action-period"> / {tier.period}</span>}
                  </p>
                  <p className="pricing-action-summary">{tier.summary}</p>
                  {isCurrent ? (
                    <p className="st st-live pricing-action-current">{t('store.currentPlan')}</p>
                  ) : (
                    <a
                      href={`/subscribe/${tier.id}`}
                      className={tier.price === 0 ? 'btn btn-secondary btn-block' : 'btn btn-primary btn-block'}
                    >
                      {tier.price === 0
                        ? `${t('store.getStarted')} — ${tier.name}`
                        : `Pay with wallet — ${tier.name}`}
                    </a>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </section>

      <div className="container"><hr className="rule" /></div>

      {/* ── FAQ ───────────────────────────────────────────────────────────── */}
      <section className="pricing-faq" aria-labelledby="faq-heading">
        <div className="container">
          <span className="kicker">{t('store.faqKicker')}</span>
          <h2 id="faq-heading" className="pricing-section-title">{t('store.faqTitle')}</h2>
          <dl className="faq-list">
            {faqs.map((faq, i) => (
              <div key={i} className="faq-item">
                <dt className="faq-question">{faq.q}</dt>
                <dd className="faq-answer">{faq.a}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      {/* ── CTA ───────────────────────────────────────────────────────────── */}
      <section className="pricing-cta" aria-labelledby="pricing-cta-heading">
        <div className="container">
          <span className="kicker">{t('store.ctaKicker')}</span>
          <h2 id="pricing-cta-heading" className="pricing-cta-title">{t('store.ctaTitle')}</h2>
          <p className="pricing-cta-body">{t('store.ctaBody')}</p>
          <div className="cta-buttons">
            <a href="/register" className="btn btn-primary btn-lg">{t('store.ctaCreateAccount')}</a>
            <a href="/marketplace" className="btn btn-secondary btn-lg">{t('store.ctaBrowse')}</a>
          </div>
        </div>
      </section>
    </div>
  );
}
