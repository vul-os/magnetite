import './SubscriptionBadge.css';

export default function SubscriptionBadge({ tier, size = 'md', showIcon = false }) {
  const tierKey  = (tier || 'free').toLowerCase();
  const sizeClass = `badge-${size}`;

  return (
    <span className={`subscription-badge tier-${tierKey} ${sizeClass}`}>
      {showIcon && <span className="badge-icon" aria-hidden="true">◎</span>}
      {tierKey.toUpperCase()}
    </span>
  );
}
