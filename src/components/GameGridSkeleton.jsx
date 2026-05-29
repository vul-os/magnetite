import Skeleton from './Skeleton';
import './GameGridSkeleton.css';

export default function GameGridSkeleton({
  count = 8,
  className = ''
}) {
  return (
    <div className={`game-grid-skeleton ${className}`}>
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="skeleton-game-card">
          <div className="skeleton-card-image" />
          <div className="skeleton-card-content">
            <Skeleton variant="text" width="85%" height={20} />
            <Skeleton variant="text" width="55%" height={14} />
            <div className="skeleton-card-meta">
              <Skeleton variant="text" width={80} height={16} />
              <Skeleton variant="text" width={60} height={16} />
            </div>
            <div className="skeleton-card-footer">
              <Skeleton variant="text" width={70} height={24} />
              <Skeleton variant="text" width={90} height={14} />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
