import { memo } from 'react';
import { initialsAvatar } from '../utils/initialsAvatar';

const TOP3_BADGES = { 1: '🥇', 2: '🥈', 3: '🥉' };

function getChangeIcon(change) {
  if (change > 0) return '↑';
  if (change < 0) return '↓';
  return '–';
}

function getRankClass(rank) {
  if (rank === 1) return 'rank-gold';
  if (rank === 2) return 'rank-silver';
  if (rank === 3) return 'rank-bronze';
  return '';
}

export default memo(function LeaderboardRow({ entry, isCurrentUser = false, highlightTop3 = false }) {
  const isTop3 = highlightTop3 && entry.rank <= 3;

  return (
    <div
      className={[
        'leaderboard-row',
        isCurrentUser ? 'current-user' : '',
        isTop3 ? 'top-3' : '',
      ].filter(Boolean).join(' ')}
      role="row"
      aria-label={`Rank ${entry.rank}: ${entry.username}, score ${entry.score.toLocaleString()}`}
    >
      <div className={`rank ${getRankClass(entry.rank)}`} role="cell">
        {isTop3 ? (
          <span aria-label={`Rank ${entry.rank}`}>{TOP3_BADGES[entry.rank]}</span>
        ) : (
          `#${entry.rank}`
        )}
      </div>

      <div className="player-info" role="cell">
        <img
          src={entry.avatar || initialsAvatar(entry.username)}
          alt={`${entry.username} avatar`}
          className="player-avatar"
          loading="lazy"
        />
        <span className="username">{entry.username}</span>
      </div>

      <div className="score" role="cell">
        {entry.score.toLocaleString()}
      </div>

      {entry.change !== undefined && (
        <div
          className={`change ${entry.change > 0 ? 'positive' : entry.change < 0 ? 'negative' : ''}`}
          role="cell"
          aria-label={`Rank change: ${entry.change > 0 ? 'up' : entry.change < 0 ? 'down' : 'unchanged'} ${Math.abs(entry.change)}`}
        >
          {getChangeIcon(entry.change)} {Math.abs(entry.change || 0)}
        </div>
      )}
    </div>
  );
});
