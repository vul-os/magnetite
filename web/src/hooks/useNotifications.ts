import { useNotificationContext } from '../context/NotificationContext';

// Re-exports the NotificationContext for convenience.
// Real-time notification updates arrive via the comms WebSocket (NotificationContext
// fetches on mount; WS-driven updates can push via addNotification).
// No polling interval needed — polling without an API call is a no-op anti-pattern.
export function useNotifications() {
  return useNotificationContext();
}
