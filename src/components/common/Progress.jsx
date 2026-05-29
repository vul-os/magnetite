import './Progress.css';

const variantClasses = {
  linear: 'progress-linear',
  circular: 'progress-circular',
};

const colorClasses = {
  primary: 'progress-primary',
  success: 'progress-success',
  warning: 'progress-warning',
  danger: 'progress-danger',
};

const sizeClasses = {
  sm: 'progress-sm',
  md: 'progress-md',
  lg: 'progress-lg',
};

export default function Progress({
  value = 0,
  variant = 'linear',
  showLabel = false,
  color = 'primary',
  size = 'md',
  className = '',
}) {
  const clampedValue = Math.min(100, Math.max(0, value));
  
  const classes = [
    'progress',
    variantClasses[variant],
    colorClasses[color],
    sizeClasses[size],
    className,
  ].filter(Boolean).join(' ');

  if (variant === 'circular') {
    const radius = 40;
    const circumference = 2 * Math.PI * radius;
    const strokeDashoffset = circumference - (clampedValue / 100) * circumference;

    return (
      <div className={classes}>
        <svg className="progress-svg" viewBox="0 0 100 100">
          <circle
            className="progress-track"
            cx="50"
            cy="50"
            r={radius}
            fill="none"
          />
          <circle
            className="progress-fill"
            cx="50"
            cy="50"
            r={radius}
            fill="none"
            strokeDasharray={circumference}
            strokeDashoffset={strokeDashoffset}
            transform="rotate(-90 50 50)"
          />
        </svg>
        {showLabel && (
          <span className="progress-label">{Math.round(clampedValue)}%</span>
        )}
      </div>
    );
  }

  return (
    <div className={classes}>
      <div className="progress-track">
        <div
          className="progress-fill"
          style={{ width: `${clampedValue}%` }}
        />
      </div>
      {showLabel && (
        <span className="progress-label">{Math.round(clampedValue)}%</span>
      )}
    </div>
  );
}
