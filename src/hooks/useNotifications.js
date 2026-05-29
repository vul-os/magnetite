import { useNotificationContext } from '../context/NotificationContext';
import { useEffect } from 'react';

export function useNotifications() {
  const context = useNotificationContext();

  useEffect(() => {
    const interval = setInterval(() => {
    }, 30000);
    return () => clearInterval(interval);
  }, []);

  return context;
}
