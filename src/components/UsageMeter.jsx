import './UsageMeter.css';

const WARNING_THRESHOLD = 0.25;
const CRITICAL_THRESHOLD = 0.1;

export default function UsageMeter({ 
  used, 
  limit, 
  unit = 'hours',
  showWarning = true 
}) {
  const remaining = Math.max(0, limit - used);
  const percentage = Math.min((used / limit) * 100, 100);
  const remainingPercentage = 100 - percentage;

  const getStatus = () => {
    if (remainingPercentage <= CRITICAL_THRESHOLD * 100) return 'critical';
    if (remainingPercentage <= WARNING_THRESHOLD * 100) return 'warning';
    return 'normal';
  };

  const status = getStatus();

  return (
    <div className={`usage-meter ${status}`}>
      <div className="usage-meter-header">
        <span className="usage-label">Hours This Month</span>
        <span className={`usage-remaining ${status}`}>
          {remaining} {unit} remaining
        </span>
      </div>

      <div className="usage-progress">
        <div className="usage-bar">
          <div 
            className="usage-fill"
            style={{ width: `${percentage}%` }}
          />
        </div>
        <span className="usage-text">{used} / {limit} {unit} used</span>
      </div>

      {showWarning && status !== 'normal' && (
        <div className={`usage-warning ${status}`}>
          {status === 'critical' ? (
            <span>⚠️ You've almost reached your limit. Upgrade now!</span>
          ) : (
            <span>⚠️ Running low on hours. Consider upgrading.</span>
          )}
        </div>
      )}
    </div>
  );
}
