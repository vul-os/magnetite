import { memo } from 'react';

export default memo(function FriendCard({ friend, onInvite, onBlock, showActions = true }) {
  const statusClass = friend.status === 'online' ? 'status-online' : 'status-offline';

  return (
    <div className="friend-card">
      <div className="friend-avatar">
        <img src={friend.avatar || `https://picsum.photos/seed/${friend.id}/100/100`} alt={friend.username} loading="lazy" />
        <span className={`status-indicator ${statusClass}`}></span>
      </div>
      <div className="friend-info">
        <h4>{friend.username}</h4>
        <span className={`status-text ${statusClass}`}>
          {friend.status === 'online' ? 'Online' : 'Offline'}
        </span>
      </div>
      {showActions && (
        <div className="friend-actions">
          <button onClick={() => onInvite?.(friend)} className="btn btn-primary btn-sm">
            Invite
          </button>
          <button onClick={() => onBlock?.(friend)} className="btn btn-secondary btn-sm">
            Block
          </button>
        </div>
      )}
    </div>
  );
});