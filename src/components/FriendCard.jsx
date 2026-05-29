import { memo } from 'react';

export default memo(function FriendCard({ friend, onInvite, onBlock, showActions = true }) {
  const isOnline = friend.status === 'online';
  const statusClass = isOnline ? 'status-online' : 'status-offline';

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
          aria-label={isOnline ? 'Online' : 'Offline'}
        />
      </div>

      <div className="friend-info">
        <h4>{friend.username}</h4>
        <span className={`status-text ${statusClass}`}>
          {isOnline ? 'Online' : 'Offline'}
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
