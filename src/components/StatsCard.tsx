export default function StatsCard({ icon, label, value, change, trend }) {
  const isPositive = change >= 0;
  const trendColor = isPositive ? 'var(--color-success)' : 'var(--color-error)';

  return (
    <div className="stats-card">
      {icon && <div className="stats-card-icon" aria-hidden="true">{icon}</div>}
      <div className="stats-card-content">
        <span className="stats-card-label">{label}</span>
        <span className="stats-card-value">{value}</span>
        {change !== undefined && (
          <span className="stats-card-change" style={{ color: trendColor }}>
            <span className="stats-card-arrow" aria-hidden="true">{isPositive ? '↑' : '↓'}</span>
            {Math.abs(change).toFixed(1)}%
            {trend && <span className="stats-card-trend">{trend}</span>}
          </span>
        )}
      </div>
    </div>
  );
}
