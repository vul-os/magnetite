import LoadingSpinner from './LoadingSpinner';
import './PageLoader.css';

export default function PageLoader({
  progress,
  message = 'Loading...',
  className = ''
}) {
  return (
    <div className={`page-loader ${className}`}>
      <div className="page-loader-content">
        <div className="page-loader-logo">
          <div className="logo-icon">
            <svg viewBox="0 0 32 32" fill="currentColor">
              <path d="M16 2L4 8v16l12 6 12-6V8L16 2zm0 4l8 4v10l-8 4-8-4V10l8-4z" />
              <path d="M16 10l-6 3v6l6 3 6-3v-6l-6-3z" opacity="0.6" />
              <circle cx="16" cy="16" r="3" />
            </svg>
          </div>
          <div className="logo-pulse" />
        </div>
        <p className="page-loader-message">{message}</p>
        {typeof progress === 'number' && (
          <div className="page-loader-progress">
            <div className="progress-bar">
              <div
                className="progress-fill"
                style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
              />
            </div>
            <span className="progress-text">{Math.round(progress)}%</span>
          </div>
        )}
        <LoadingSpinner size="sm" color="primary" />
      </div>
    </div>
  );
}
