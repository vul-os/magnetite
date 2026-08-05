import type { ComponentType } from 'react';
import { TrophyIcon, UsersIcon, WalletIcon, SettingsIcon } from '../assets/icons';
import type { AppNotification } from '../context/NotificationContext';

const typeIcons: Record<string, ComponentType<{ width?: number; height?: number }>> = {
  achievement: TrophyIcon,
  invite: UsersIcon,
  payout: WalletIcon,
  system: SettingsIcon,
};

interface NotificationItemProps {
  notification: AppNotification;
  onClick: (notification: AppNotification) => void;
}

export default function NotificationItem({ notification, onClick }: NotificationItemProps) {
  const Icon = typeIcons[notification.type ?? ''] || SettingsIcon;

  const timeAgo = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const seconds = Math.floor((now.getTime() - date.getTime()) / 1000);

    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`;
    return date.toLocaleDateString();
  };

  return (
    <div
      className={`notification-item ${notification.read ? '' : 'unread'}`}
      onClick={() => onClick(notification)}
    >
      <div className={`notification-icon ${notification.type}`}>
        <Icon width={16} height={16} />
      </div>
      <div className="notification-content">
        <div className="notification-title">{notification.title}</div>
        <div className="notification-message">{notification.message}</div>
        <div className="notification-time">{timeAgo(notification.createdAt)}</div>
      </div>
      {!notification.read && <div className="unread-dot" />}
    </div>
  );
}
