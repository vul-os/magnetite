import { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';

const NotificationContext = createContext();

/* Mock data — only used when VITE_USE_MOCKS=true */
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export function NotificationProvider({ children }) {
  const [notifications, setNotifications] = useState([]);
  const [initialized, setInitialized] = useState(false);

  // Load from real API on mount (or mock data if VITE_USE_MOCKS=true)
  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        if (USE_MOCKS) {
          const { mockNotifications } = await import('../data/mockNotifications');
          if (!cancelled) setNotifications(mockNotifications);
          return;
        }
        const data = await api.notifications.list();
        if (!cancelled) {
          const list = Array.isArray(data) ? data : (data?.notifications ?? null);
          if (list) setNotifications(list);
        }
      } catch {
        /* API unavailable — empty list is the honest state; no fabricated data shown */
      } finally {
        if (!cancelled) setInitialized(true);
      }
    }
    load();
    return () => { cancelled = true; };
  }, []);

  const unreadCount = notifications.filter(n => !n.read).length;

  const markAsRead = useCallback((id) => {
    api.notifications.markAsRead(id).catch(() => { /* optimistic */ });
    setNotifications(prev =>
      prev.map(n => n.id === id ? { ...n, read: true } : n)
    );
  }, []);

  const markAllAsRead = useCallback(() => {
    api.notifications.markAllAsRead().catch(() => { /* optimistic */ });
    setNotifications(prev => prev.map(n => ({ ...n, read: true })));
  }, []);

  const addNotification = useCallback((notification) => {
    const newNotification = {
      id: Date.now().toString(),
      read: false,
      createdAt: new Date().toISOString(),
      ...notification,
    };
    setNotifications(prev => [newNotification, ...prev]);
  }, []);

  return (
    <NotificationContext.Provider value={{
      notifications,
      unreadCount,
      markAsRead,
      markAllAsRead,
      addNotification,
      initialized,
    }}>
      {children}
    </NotificationContext.Provider>
  );
}

export function useNotificationContext() {
  const context = useContext(NotificationContext);
  if (!context) {
    throw new Error('useNotificationContext must be used within NotificationProvider');
  }
  return context;
}
