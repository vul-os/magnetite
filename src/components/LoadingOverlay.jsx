import { useEffect, useState } from 'react';
import LoadingSpinner from './LoadingSpinner';
import './LoadingOverlay.css';

export default function LoadingOverlay({
  visible = true,
  message = '',
  fullScreen = true,
  className = ''
}) {
  const [shouldRender, setShouldRender] = useState(visible);

  useEffect(() => {
    if (visible) {
      setShouldRender(true);
    } else {
      const timer = setTimeout(() => setShouldRender(false), 300);
      return () => clearTimeout(timer);
    }
  }, [visible]);

  if (!shouldRender) return null;

  const classes = [
    'loading-overlay',
    visible ? 'visible' : 'hidden',
    fullScreen ? 'fullscreen' : 'container',
    className
  ].filter(Boolean).join(' ');

  return (
    <div className={classes}>
      <div className="loading-overlay-content">
        <LoadingSpinner size="lg" color="primary" />
        {message && <p className="loading-message">{message}</p>}
      </div>
    </div>
  );
}
