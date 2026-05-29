import { useAnnouncementContext } from '../context/AnnouncementContext';
import './AnnouncementBanner.css';

export function useAnnouncement() {
  const { announcement, isVisible, isDismissed, dismiss, show } = useAnnouncementContext();
  return {
    announcement,
    isVisible,
    isDismissed,
    dismiss,
    show,
  };
}

export default function AnnouncementBanner() {
  const { announcement, isVisible, dismiss } = useAnnouncement();

  if (!isVisible || !announcement) return null;

  return (
    <div className="announcement-banner" role="alert">
      <div className="announcement-content">
        <div className="announcement-icon">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <path d="M10 2L1 18h18L10 2z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
            <path d="M10 8v4M10 14v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
          </svg>
        </div>
        <span className="announcement-text">{announcement}</span>
      </div>
      <button className="announcement-dismiss" onClick={dismiss} aria-label="Dismiss announcement">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
        </svg>
      </button>
    </div>
  );
}
