import './LeaderboardSkeleton.css';

export default function LeaderboardSkeleton() {
  return (
    <div className="leaderboard-skeleton">
      <div className="skeleton-rank" />
      <div className="skeleton-avatar" />
      <div className="skeleton-info">
        <div className="skeleton-name" />
        <div className="skeleton-stat" />
      </div>
      <div className="skeleton-score" />
    </div>
  );
}
