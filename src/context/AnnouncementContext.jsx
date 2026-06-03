import { createContext, useContext, useState, useCallback, useEffect } from 'react';

const AnnouncementContext = createContext();

const STORAGE_KEY = 'magnetite_announcement_dismissed';

export function AnnouncementProvider({ children, announcement = null }) {
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      return stored ? JSON.parse(stored) : false;
    } catch {
      return false;
    }
  });

  // Reconcile visibility whenever the announcement prop or dismissed state
  // changes. dismiss()/show() also imperatively toggle visibility, so a single
  // source of truth via an effect keeps the two paths consistent.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setIsVisible(Boolean(announcement) && !isDismissed);
  }, [announcement, isDismissed]);

  const dismiss = useCallback(() => {
    setIsDismissed(true);
    setIsVisible(false);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(true));
    } catch { /* storage unavailable */ }
  }, []);

  const show = useCallback(() => {
    setIsDismissed(false);
    setIsVisible(true);
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch { /* storage unavailable */ }
  }, []);

  return (
    <AnnouncementContext.Provider value={{ announcement, isVisible, isDismissed, dismiss, show }}>
      {children}
    </AnnouncementContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useAnnouncementContext() {
  const context = useContext(AnnouncementContext);
  if (!context) throw new Error('useAnnouncementContext must be used within AnnouncementProvider');
  return context;
}
