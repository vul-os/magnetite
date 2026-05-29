import EmptyState from './EmptyState';
import Button from '../common/Button';

const TrophyIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M8 21h8" />
    <path d="M12 17v4" />
    <path d="M7 4H4a2 2 0 0 0-2 2v2c0 2.2 1.8 4 4 4h1" />
    <path d="M17 4h3a2 2 0 0 1 2 2v2c0 2.2-1.8 4-4 4h-1" />
    <path d="M7 4v8a5 5 0 0 0 10 0V4" />
  </svg>
);

export default function NoAchievements({ category, onReset }) {
  return (
    <EmptyState
      icon={<TrophyIcon />}
      title={category && category !== 'all' ? 'No achievements here' : 'No achievements yet'}
      description={
        category && category !== 'all'
          ? `No ${category} achievements found. Try a different category or keep playing.`
          : 'Play games to start earning achievements and filling your trophy room.'
      }
      action={
        onReset ? (
          <Button variant="secondary" onClick={onReset}>
            Show All
          </Button>
        ) : null
      }
    />
  );
}
