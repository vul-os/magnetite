import { memo } from 'react';

export default memo(function LeaderboardRow({ entry, isCurrentUser = false, highlightTop3 = false }) {
  const isTop3 = highlightTop3 && entry.rank <= 3;
  const rankClass = entry.rank === 1 ? 'rank-gold' : entry.rank === 2 ? 'rank-silver' : entry.rank === 3 ? 'rank-bronze' : '';

  const getChangeIcon = () => {
    if (entry.change > 0) return '↑';
    if (entry.change < 0) return '↓';
    return '→';
  };

  return (
    <div className={`leaderboard-row ${isCurrentUser ? 'current-user' : ''} ${isTop3 ? 'top-3' : ''}`}>
      <div className={`rank ${rankClass}`}>
        {isTop3 ? getTop3Badge(entry.rank) : `#${entry.rank}`}
      </div>
      <div className="player-info">
        <img
          src={entry.avatar || `https://picsum.photos/seed/${entry.username}/100/100`}
          alt={entry.username}
          className="player-avatar"
          loading="lazy"
        />
        <span className="username">{entry.username}</span>
      </div>
      <div className="score">{entry.score.toLocaleString()}</div>
      {entry.change !== undefined && (
        <div className={`change ${entry.change > 0 ? 'positive' : entry.change < 0 ? 'negative' : ''}`}>
          {getChangeIcon()} {Math.abs(entry.change || 0)}
        </div>
      )}
    </div>
  );
});

function getTop3Badge(rank) {
  const badges = { 1: '🥇', 2: '🥈', 3: '🥉' };
  return badges[rank] || rank;
}
