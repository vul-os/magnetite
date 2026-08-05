import GameCardSkeleton from './GameCardSkeleton';
import './GameGridSkeleton.css';

interface GameGridSkeletonProps {
  count?: number;
}

export default function GameGridSkeleton({ count = 6 }: GameGridSkeletonProps) {
  return (
    <div className="game-grid-skeleton">
      {Array.from({ length: count }).map((_, i) => (
        <GameCardSkeleton key={i} />
      ))}
    </div>
  );
}
