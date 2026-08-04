import { TrophyIcon, UsersIcon, WalletIcon, SettingsIcon } from '../assets/icons';

const typeIcons = {
  achievement: TrophyIcon,
  invite: UsersIcon,
  payout: WalletIcon,
  system: SettingsIcon,
};

export default function NotificationItem({ notification, onClick }) {
  const Icon = typeIcons[notification.type] || SettingsIcon;

  const timeAgo = (dateString) => {
    const date = new Date(dateString);
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

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
