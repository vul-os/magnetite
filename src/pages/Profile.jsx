import { useState } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import ProfileCard from '../components/ProfileCard';
import { mockProfileUser, mockRecentGames, mockProfileAchievements, mockProfileFriends } from '../data/mockProfile';
import './social.css';

export default function Profile() {
  const [isFollowing, setIsFollowing] = useState(false);
  const [user] = useState(mockProfileUser);

  return (
    <Layout>
      <div className="profile-page">
        <ProfileCard
          user={user}
          isOwnProfile={true}
          isFollowing={isFollowing}
          onEdit={() => window.location.href = '/edit-profile'}
          onFollow={() => setIsFollowing(true)}
          onUnfollow={() => setIsFollowing(false)}
        />

        <div className="profile-sections">
          <section className="profile-section">
            <div className="section-header">
              <h3>Recent Games</h3>
              <Link to="/leaderboard" className="view-all">View All</Link>
            </div>
            <div className="recent-games-grid">
              {mockRecentGames.map(game => (
                <div key={game.id} className="recent-game-card">
                  <img src={game.thumbnail} alt={game.title} loading="lazy" />
                  <div className="recent-game-info">
                    <h4>{game.title}</h4>
                    <p>Score: {game.score.toLocaleString()}</p>
                    <p>Rank: #{game.rank}</p>
                    <span className="played-date">
                      {new Date(game.playedAt).toLocaleDateString()}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="profile-section">
            <div className="section-header">
              <h3>Achievements</h3>
              <Link to="/achievements" className="view-all">View All</Link>
            </div>
            <div className="achievements-preview">
              {mockProfileAchievements.map(achievement => (
                <div key={achievement.id} className="achievement-preview-item">
                  <span className="achievement-icon">{achievement.icon}</span>
                  <span className="achievement-name">{achievement.name}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="profile-section">
            <div className="section-header">
              <h3>Friends</h3>
              <Link to="/friends" className="view-all">View All</Link>
            </div>
            <div className="friends-preview">
              {mockProfileFriends.map(friend => (
                <Link
                  key={friend.id}
                  to={`/profile/${friend.username}`}
                  className="friend-preview-item"
                >
                  <img src={friend.avatar} alt={friend.username} loading="lazy" />
                  <span className="friend-name">{friend.username}</span>
                  <span className={`friend-status ${friend.status}`} aria-hidden="true" />
                </Link>
              ))}
            </div>
          </section>
        </div>
      </div>
    </Layout>
  );
}
