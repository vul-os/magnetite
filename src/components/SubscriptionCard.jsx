import './SubscriptionCard.css';

/* Map tier names to CSS variable names for token-based colouring */
const TIER_CLASS = {
  free:      'tier-free',
  basic:     'tier-basic',
  pro:       'tier-pro',
  unlimited: 'tier-unlimited',
};

export default function SubscriptionCard({ tier, isCurrent, onSubscribe }) {
  const {
    name,
    price,
    period,
    features,
    recommended = false
  } = tier;

  const tierKey  = name.toLowerCase();
  const tierClass = TIER_CLASS[tierKey] || TIER_CLASS.free;

  function getButtonLabel(tierName) {
    const tierOrder = ['Free', 'Basic', 'Pro', 'Unlimited'];
    const targetIndex = tierOrder.indexOf(tierName);
    const currentIndex = tierOrder.indexOf('Free');
    if (targetIndex > currentIndex) {
      return `Upgrade to ${tierName}`;
    }
    return `Subscribe to ${tierName}`;
  }

  return (
    <div className={`subscription-card ${tierClass} ${isCurrent ? 'current' : ''} ${recommended ? 'recommended' : ''}`}>
      {recommended && <div className="subscription-card-badge">Recommended</div>}

      <div className="subscription-card-header">
        <div className="subscription-tier-indicator" />
        <h3 className="subscription-tier-name">{name}</h3>
        <div className="subscription-price">
          <span className="price-amount">{price}</span>
          {period && <span className="price-period">/{period}</span>}
        </div>
      </div>

      <ul className="subscription-features">
        {features.map((feature, index) => (
          <li key={index} className="subscription-feature">
            <span className="feature-check" aria-hidden="true">✓</span>
            <span>{feature}</span>
          </li>
        ))}
      </ul>

      <div className="subscription-card-footer">
        {isCurrent ? (
          <button className="btn btn-secondary subscription-btn" disabled aria-label="Current plan">
            Current Plan
          </button>
        ) : (
          <button
            className={`btn ${recommended ? 'btn-primary' : 'btn-secondary'} subscription-btn`}
            onClick={() => onSubscribe(tier)}
          >
            {getButtonLabel(name)}
          </button>
        )}
      </div>
    </div>
  );
}
