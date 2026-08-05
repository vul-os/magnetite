import './GameCardSkeleton.css';

export default function GameCardSkeleton() {
  return (
    <div className="game-card-skeleton">
      <div className="skeleton-image" />
      <div className="skeleton-content">
        <div className="skeleton-title" />
        <div className="skeleton-developer" />
        <div className="skeleton-rating" />
        <div className="skeleton-footer">
          <div className="skeleton-price" />
        </div>
      </div>
    </div>
  );
}
