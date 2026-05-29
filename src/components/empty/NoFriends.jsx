import EmptyState from './EmptyState';
import Button from '../common/Button';

const UsersIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="9" cy="7" r="4" />
    <path d="M3 21v-2a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v2" />
    <circle cx="17" cy="7" r="3" opacity="0.6" />
    <path d="M21 21v-2a3 3 0 0 0-2-2.83" opacity="0.6" />
  </svg>
);

export default function NoFriends({ action }) {
  return (
    <EmptyState
      icon={<UsersIcon />}
      title="No friends yet"
      description="Add friends to play together and enjoy the experience."
      action={action || <Button>Add Friends</Button>}
    />
  );
}
