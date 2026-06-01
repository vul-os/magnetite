import { createContext, useContext, useState, useCallback, useEffect, useRef } from 'react';
import { api } from '../api/client';

const NotificationContext = createContext();

/* Mock data — only used when VITE_USE_MOCKS=true */
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

function getWsBase() {
  const apiBase = import.meta.env.VITE_API_URL || 'http://localhost:8080';
  return apiBase.replace(/^http/, 'ws');
}

export function NotificationProvider({ children }) {
  const [notifications, setNotifications] = useState([]);
  const [initialized, setInitialized] = useState(false);
  const wsRef = useRef(null);
  const reconnectTimerRef = useRef(null);
  const mountedRef = useRef(true);

  const addNotification = useCallback((notification) => {
    const newNotification = {
      id: Date.now().toString(),
      read: false,
      createdAt: new Date().toISOString(),
      ...notification,
    };
    setNotifications(prev => [newNotification, ...prev]);
  }, []);

  // Load from real API on mount (or mock data if VITE_USE_MOCKS=true)
  useEffect(() => {
    mountedRef.current = true;
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

  // Real-time notification WebSocket connection
  useEffect(() => {
    if (USE_MOCKS) return;

    function connect() {
      const token = localStorage.getItem('token');
      if (!token) return; // Not authenticated — don't connect

      const wsBase = getWsBase();
      const wsUrl = `${wsBase}/ws/notifications?token=${encodeURIComponent(token)}`;

      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        // Send subscribe action to confirm we're listening
        ws.send(JSON.stringify({ action: 'subscribe' }));
      };

      ws.onmessage = (event) => {
        if (!mountedRef.current) return;
        try {
          const msg = JSON.parse(event.data);
          // Backend sends: { user_id, notification: { id, type, title, body, data, created_at } }
          const notif = msg.notification ?? msg;
          if (notif && (notif.title || notif.type)) {
            setNotifications(prev => {
              // Avoid duplicate if already in list
              if (notif.id && prev.some(n => n.id === notif.id)) return prev;
              return [{
                id: notif.id ?? Date.now().toString(),
                read: false,
                title: notif.title ?? '',
                body: notif.body ?? null,
                notification_type: notif.type ?? notif.notification_type ?? '',
                created_at: notif.created_at ?? new Date().toISOString(),
                createdAt: notif.created_at ?? new Date().toISOString(),
                data: notif.data ?? null,
              }, ...prev];
            });
          }
        } catch {
          /* ignore parse errors */
        }
      };

      ws.onerror = () => {
        /* Connection error — will retry on close */
      };

      ws.onclose = () => {
        wsRef.current = null;
        if (!mountedRef.current) return;
        // Reconnect after 5 seconds
        reconnectTimerRef.current = setTimeout(() => {
          if (mountedRef.current) connect();
        }, 5000);
      };
    }

    connect();

    return () => {
      mountedRef.current = false;
      clearTimeout(reconnectTimerRef.current);
      if (wsRef.current) {
        wsRef.current.onclose = null; // Prevent reconnect on intentional close
        wsRef.current.close();
        wsRef.current = null;
      }
    };
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
