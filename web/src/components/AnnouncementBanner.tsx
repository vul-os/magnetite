import { useAnnouncementContext } from '../context/AnnouncementContext';
import './AnnouncementBanner.css';

export default function AnnouncementBanner() {
  const { announcement, isVisible, dismiss } = useAnnouncementContext();

  if (!isVisible || !announcement) return null;

  return (
    <div className="announcement-banner" role="alert" aria-live="polite">
      <div className="announcement-content">
        <div className="announcement-icon" aria-hidden="true">
          <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
            <path d="M10 2L1 18h18L10 2z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
            <path d="M10 8v4M10 14v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
          </svg>
        </div>
        <span className="announcement-text">{announcement}</span>
      </div>
      <button
        className="announcement-dismiss"
        onClick={dismiss}
        aria-label="Dismiss announcement"
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
        </svg>
      </button>
    </div>
  );
}
