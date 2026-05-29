import GameCardSkeleton from './GameCardSkeleton';
import './GameGridSkeleton.css';

export default function GameGridSkeleton({ count = 6 }) {
  return (
    <div className="game-grid-skeleton">
      {Array.from({ length: count }).map((_, i) => (
        <GameCardSkeleton key={i} />
      ))}
    </div>
  );
}
