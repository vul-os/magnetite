import EmptyState from './EmptyState';
import Button from '../common/Button';

const GameControllerIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <rect x="2" y="6" width="20" height="12" rx="4" />
    <path d="M6 12h4" />
    <path d="M8 10v4" />
    <circle cx="16" cy="10" r="1" fill="currentColor" />
    <circle cx="18" cy="12" r="1" fill="currentColor" />
  </svg>
);

export default function NoGames({ action }) {
  return (
    <EmptyState
      icon={<GameControllerIcon />}
      title="No games yet"
      description="Be the first to host a game and start playing with others."
      action={action || <Button>Host a Game</Button>}
    />
  );
}
