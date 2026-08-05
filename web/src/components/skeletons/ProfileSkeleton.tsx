import './ProfileSkeleton.css';

export default function ProfileSkeleton() {
  return (
    <div className="profile-skeleton">
      <div className="skeleton-avatar" />
      <div className="skeleton-name" />
      <div className="skeleton-bio">
        <div className="skeleton-bio-line" />
        <div className="skeleton-bio-line" />
        <div className="skeleton-bio-line" />
      </div>
      <div className="skeleton-stats">
        <div className="skeleton-stat-item">
          <div className="skeleton-stat-value" />
          <div className="skeleton-stat-label" />
        </div>
        <div className="skeleton-stat-item">
          <div className="skeleton-stat-value" />
          <div className="skeleton-stat-label" />
        </div>
        <div className="skeleton-stat-item">
          <div className="skeleton-stat-value" />
          <div className="skeleton-stat-label" />
        </div>
      </div>
    </div>
  );
}
