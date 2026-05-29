import './SubscriptionBadge.css';

const TIER_CONFIG = {
  free: {
    label: 'FREE',
    color: '#6b7280',
    bgColor: 'rgba(107, 114, 128, 0.1)',
  },
  basic: {
    label: 'BASIC',
    color: '#3b82f6',
    bgColor: 'rgba(59, 130, 246, 0.1)',
  },
  pro: {
    label: 'PRO',
    color: '#8b5cf6',
    bgColor: 'rgba(139, 92, 246, 0.1)',
  },
  unlimited: {
    label: 'UNLIMITED',
    color: '#f59e0b',
    bgColor: 'rgba(245, 158, 11, 0.1)',
  },
};

export default function SubscriptionBadge({ tier, size = 'md', showIcon = false }) {
  const config = TIER_CONFIG[tier?.toLowerCase()] || TIER_CONFIG.free;

  const sizeClass = `badge-${size}`;

  return (
    <span 
      className={`subscription-badge ${sizeClass}`}
      style={{ 
        color: config.color,
        backgroundColor: config.bgColor,
        borderColor: config.color,
      }}
    >
      {showIcon && <span className="badge-icon">◎</span>}
      {config.label}
    </span>
  );
}
