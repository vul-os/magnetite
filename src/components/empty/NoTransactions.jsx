import EmptyState from './EmptyState';
import Button from '../common/Button';

const WalletIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4" />
    <path d="M3 5v14a2 2 0 0 0 2 2h16v-5" />
    <path d="M18 12a2 2 0 0 0 0 4h4v-4Z" />
    <circle cx="18" cy="12" r="1" fill="currentColor" opacity="0.5" />
  </svg>
);

export default function NoTransactions({ action }) {
  return (
    <EmptyState
      icon={<WalletIcon />}
      title="No transactions yet"
      description="Make your first deposit to start playing and earning."
      action={action || <Button>Make a Deposit</Button>}
    />
  );
}
