import { memo } from 'react';
import Button from './common/Button';

export default memo(function PlayerList({
  players,
  hostId,
  currentUserId,
  onKickPlayer,
}) {
  if (!players || players.length === 0) {
    return (
      <div className="player-list-empty">
        <p>Waiting for players to join...</p>
      </div>
    );
  }

  return (
    <div className="player-list">
      <div className="player-list-header">
        <h3>Players</h3>
        <span className="player-count">{players.length}</span>
      </div>
      <div className="player-list-content">
        {players.map((player) => {
          const isHost = player.id === hostId;
          const isCurrentUser = player.id === currentUserId;

          return (
            <div
              key={player.id}
              className={`player-row ${isCurrentUser ? 'current-user' : ''}`}
            >
              <div className="player-avatar">
                {player.avatar ? (
                  <img src={player.avatar} alt={player.username} loading="lazy" />
                ) : (
                  <div className="avatar-placeholder">
                    {player.username?.charAt(0)?.toUpperCase() || '?'}
                  </div>
                )}
                <span className={`ready-indicator ${player.ready ? 'ready' : ''}`} />
              </div>
              <div className="player-info">
                <span className="player-name">
                  {player.username}
                  {isHost && <span className="host-badge">Host</span>}
                </span>
              </div>
              {isHost && !isCurrentUser && onKickPlayer && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onKickPlayer(player.id)}
                  className="kick-btn"
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                    <path d="M4 4l6 6M10 4l-6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                  </svg>
                </Button>
              )}
            </div>
          );
        })}
      </div>

      <style>{`
        .player-list {
          background: rgba(10, 10, 15, 0.8);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .player-list-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem;
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }
        .player-list-header h3 {
          margin: 0;
          font-size: 0.875rem;
          font-weight: 600;
          color: var(--color-text-primary, #fff);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .player-count {
          display: flex;
          align-items: center;
          justify-content: center;
          min-width: 24px;
          height: 24px;
          padding: 0 6px;
          background: var(--color-accent, #8b5cf6);
          border-radius: 12px;
          font-size: 0.75rem;
          font-weight: 600;
          color: #fff;
        }
        .player-list-content {
          max-height: 320px;
          overflow-y: auto;
        }
        .player-row {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.75rem 1rem;
          transition: background 0.15s;
        }
        .player-row:hover {
          background: rgba(255, 255, 255, 0.03);
        }
        .player-row.current-user {
          background: rgba(139, 92, 246, 0.1);
        }
        .player-avatar {
          position: relative;
          width: 40px;
          height: 40px;
          flex-shrink: 0;
        }
        .player-avatar img {
          width: 100%;
          height: 100%;
          object-fit: cover;
          border-radius: 50%;
        }
        .avatar-placeholder {
          width: 100%;
          height: 100%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: linear-gradient(135deg, var(--color-accent, #8b5cf6), #6366f1);
          border-radius: 50%;
          font-size: 1rem;
          font-weight: 600;
          color: #fff;
        }
        .ready-indicator {
          position: absolute;
          bottom: 0;
          right: 0;
          width: 12px;
          height: 12px;
          background: var(--color-text-muted, #666);
          border: 2px solid var(--color-bg-primary, #0a0a0f);
          border-radius: 50%;
        }
        .ready-indicator.ready {
          background: var(--color-success, #22c55e);
        }
        .player-info {
          flex: 1;
          min-width: 0;
        }
        .player-name {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          font-size: 0.9375rem;
          font-weight: 500;
          color: var(--color-text-primary, #fff);
        }
        .host-badge {
          display: inline-flex;
          padding: 2px 6px;
          background: rgba(251, 191, 36, 0.2);
          border: 1px solid rgba(251, 191, 36, 0.4);
          border-radius: 4px;
          font-size: 0.625rem;
          font-weight: 600;
          color: #fbbf24;
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .kick-btn {
          opacity: 0;
          transition: opacity 0.15s;
        }
        .player-row:hover .kick-btn {
          opacity: 1;
        }
        .player-list-empty {
          padding: 2rem;
          text-align: center;
          color: var(--color-text-muted, #666);
        }
        .player-list-empty p {
          margin: 0;
        }
      `}</style>
    </div>
  );
});