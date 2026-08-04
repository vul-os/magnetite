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

  // Mount immediately when shown, but defer unmount by 300ms so the exit
  // animation can play. The delayed unmount is an intentional synchronization
  // with the CSS transition, which requires driving render state from an effect.
  useEffect(() => {
    if (visible) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
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
