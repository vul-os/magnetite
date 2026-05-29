import './SubscriptionCard.css';

const TIER_COLORS = {
  free: '#6b7280',
  basic: '#3b82f6',
  pro: '#8b5cf6',
  unlimited: '#f59e0b',
};

export default function SubscriptionCard({ tier, isCurrent, onSubscribe }) {
  const {
    name,
    price,
    period,
    features,
    recommended = false
  } = tier;

  const tierKey = name.toLowerCase();
  const accentColor = TIER_COLORS[tierKey] || TIER_COLORS.free;

  return (
    <div className={`subscription-card ${isCurrent ? 'current' : ''} ${recommended ? 'recommended' : ''}`}>
      {recommended && <div className="subscription-card-badge">Recommended</div>}
      
      <div className="subscription-card-header" style={{ borderColor: accentColor }}>
        <div className="subscription-tier-indicator" style={{ backgroundColor: accentColor }} />
        <h3 className="subscription-tier-name">{name}</h3>
        <div className="subscription-price">
          <span className="price-amount">{price}</span>
          {period && <span className="price-period">/{period}</span>}
        </div>
      </div>

      <ul className="subscription-features">
        {features.map((feature, index) => (
          <li key={index} className="subscription-feature">
            <span className="feature-check">✓</span>
            <span>{feature}</span>
          </li>
        ))}
      </ul>

      <div className="subscription-card-footer">
        {isCurrent ? (
          <button className="btn btn-secondary subscription-btn" disabled>
            Current Plan
          </button>
        ) : (
          <button 
            className={`btn ${recommended ? 'btn-primary' : 'btn-outline'} subscription-btn`}
            onClick={() => onSubscribe(tier)}
          >
            {getButtonLabel(name)}
          </button>
        )}
      </div>
    </div>
  );

  function getButtonLabel(tierName) {
    const tierOrder = ['Free', 'Basic', 'Pro', 'Unlimited'];
    const currentIndex = tierOrder.indexOf('Free');
    const targetIndex = tierOrder.indexOf(tierName);
    
    if (targetIndex > currentIndex) {
      return `Upgrade to ${tierName}`;
    }
    return `Subscribe to ${tierName}`;
  }
}
