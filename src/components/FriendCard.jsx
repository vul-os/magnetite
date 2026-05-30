import { memo } from 'react';

/** Map a presence status string to CSS modifier + human label. */
function resolveStatus(status) {
  switch (status) {
    case 'online':  return { cls: 'status-online',   label: 'Online' };
    case 'idle':    return { cls: 'status-idle',     label: 'Idle' };
    case 'dnd':     return { cls: 'status-dnd',      label: 'Do Not Disturb' };
    case 'offline':
    default:        return { cls: 'status-offline',  label: 'Offline' };
  }
}

export default memo(function FriendCard({ friend, onInvite, onBlock, showActions = true }) {
  const { cls: statusClass, label: statusLabel } = resolveStatus(friend.status);
  const activityText = friend.activity ?? statusLabel;

  return (
    <div className="friend-card">
      <div className="friend-avatar">
        <img
          src={friend.avatar || `https://picsum.photos/seed/${friend.id}/100/100`}
          alt={`${friend.username} avatar`}
          loading="lazy"
        />
        <span
          className={`status-indicator ${statusClass}`}
          aria-label={statusLabel}
        />
      </div>

      <div className="friend-info">
        <h4>{friend.username}</h4>
        <span className={`status-text ${statusClass}`}>
          {activityText}
        </span>
      </div>

      {showActions && (
        <div className="friend-actions">
          <button
            onClick={() => onInvite?.(friend)}
            className="btn btn-primary btn-sm"
            aria-label={`Invite ${friend.username}`}
          >
            Invite
          </button>
          <button
            onClick={() => onBlock?.(friend)}
            className="btn btn-secondary btn-sm"
            aria-label={`Block ${friend.username}`}
          >
            Block
          </button>
        </div>
      )}
    </div>
  );
});
