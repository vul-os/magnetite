import './TransactionSkeleton.css';

export default function TransactionSkeleton() {
  return (
    <div className="transaction-skeleton">
      <div className="skeleton-icon" />
      <div className="skeleton-content">
        <div className="skeleton-text" />
        <div className="skeleton-subtext" />
      </div>
      <div className="skeleton-amount" />
    </div>
  );
}
