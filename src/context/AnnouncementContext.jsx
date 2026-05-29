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

  useEffect(() => {
    if (announcement && !isDismissed) {
      setIsVisible(true);
    } else {
      setIsVisible(false);
    }
  }, [announcement, isDismissed]);

  const dismiss = useCallback(() => {
    setIsDismissed(true);
    setIsVisible(false);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(true));
    } catch {}
  }, []);

  const show = useCallback(() => {
    setIsDismissed(false);
    setIsVisible(true);
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {}
  }, []);

  return (
    <AnnouncementContext.Provider value={{ announcement, isVisible, isDismissed, dismiss, show }}>
      {children}
    </AnnouncementContext.Provider>
  );
}

export function useAnnouncementContext() {
  const context = useContext(AnnouncementContext);
  if (!context) throw new Error('useAnnouncementContext must be used within AnnouncementProvider');
  return context;
}
