import { memo } from 'react';
import Button from './common/Button';

export default memo(function ProfileCard({
  user,
  isOwnProfile = false,
  isFollowing = false,
  onEdit,
  onFollow,
  onUnfollow,
}) {
  return (
    <div className="profile-card">
      <div className="profile-cover" aria-hidden="true">
        <img src={user.coverImage} alt="" loading="lazy" />
      </div>

      <div className="profile-avatar">
        <img src={user.avatar} alt={`${user.username} avatar`} loading="lazy" />
      </div>

      <div className="profile-info">
        <div className="profile-header">
          <h2 className="profile-username">{user.username}</h2>
          <span
            className={`profile-status ${user.isOnline ? 'online' : 'offline'}`}
            aria-label={user.isOnline ? 'Online' : 'Offline'}
          >
            {user.isOnline ? 'Online' : 'Offline'}
          </span>
        </div>

        {user.bio && (
          <p className="profile-bio">{user.bio}</p>
        )}

        {user.location && (
          <p className="profile-location">
            <span aria-hidden="true">&#x1F4CD;</span> {user.location}
          </p>
        )}

        <p className="profile-joined">
          Joined {new Date(user.joinedAt).toLocaleDateString()}
        </p>
      </div>

      <div className="profile-stats" role="list" aria-label="Profile statistics">
        <div className="stat-item" role="listitem">
          <span className="stat-value">{user.stats.gamesPlayed}</span>
          <span className="stat-label">Games</span>
        </div>
        <div className="stat-item" role="listitem">
          <span className="stat-value">{user.stats.achievements}</span>
          <span className="stat-label">Achievements</span>
        </div>
        <div className="stat-item" role="listitem">
          <span className="stat-value">{user.stats.friends}</span>
          <span className="stat-label">Friends</span>
        </div>
      </div>

      <div className="profile-actions">
        {isOwnProfile ? (
          <Button variant="secondary" onClick={onEdit}>Edit Profile</Button>
        ) : isFollowing ? (
          <Button variant="secondary" onClick={onUnfollow}>Following</Button>
        ) : (
          <Button variant="primary" onClick={onFollow}>Follow</Button>
        )}
      </div>
    </div>
  );
});
