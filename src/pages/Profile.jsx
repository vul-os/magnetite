import { useState, useEffect } from 'react';
import { Link, useParams } from 'react-router-dom';
import Layout from '../components/Layout';
import ProfileCard from '../components/ProfileCard';
import { mockProfileUser, mockRecentGames, mockProfileAchievements, mockProfileFriends } from '../data/mockProfile';
import { api } from '../api/client';
import './social.css';

export default function Profile() {
  const { username } = useParams();
  const [isFollowing, setIsFollowing] = useState(false);
  const [user, setUser] = useState(mockProfileUser);
  const [recentGames, setRecentGames] = useState(mockRecentGames);
  const [achievements, setAchievements] = useState(mockProfileAchievements);
  const [friends, setFriends] = useState(mockProfileFriends);

  useEffect(() => {
    let cancelled = false;

    async function loadProfile() {
      try {
        const target = username || (await api.auth.me().then(me => me?.username).catch(() => null));
        if (!target) return;

        const data = await api.profile.get(target);
        if (!cancelled && data) {
          const userData = data.user || data;
          setUser(userData);
          if (data.recent_games) setRecentGames(data.recent_games);
          if (data.achievements) setAchievements(data.achievements.slice(0, 3));
          if (data.friends) setFriends(data.friends.slice(0, 4));
        }
      } catch { /* use mock data */ }
    }

    loadProfile();
    return () => { cancelled = true; };
  }, [username]);

  return (
    <Layout>
      <div className="profile-page reveal">
        <div className="reveal-1">
          <ProfileCard
            user={user}
            isOwnProfile={!username}
            isFollowing={isFollowing}
            onEdit={() => window.location.href = '/edit-profile'}
            onFollow={() => setIsFollowing(true)}
            onUnfollow={() => setIsFollowing(false)}
          />
        </div>

        <div className="profile-sections">
          <section className="profile-section reveal-2">
            <div className="section-header">
              <h3>// Recent Games</h3>
              <Link to="/leaderboard" className="view-all">View All →</Link>
            </div>
            <div className="recent-games-grid">
              {recentGames.map(game => (
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

          <section className="profile-section reveal-3">
            <div className="section-header">
              <h3>// Achievements</h3>
              <Link to="/achievements" className="view-all">View All →</Link>
            </div>
            <div className="achievements-preview">
              {achievements.map(achievement => (
                <div key={achievement.id} className="achievement-preview-item">
                  <span className="achievement-icon" aria-hidden="true">{achievement.icon}</span>
                  <span className="achievement-name">{achievement.name}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="profile-section reveal-4">
            <div className="section-header">
              <h3>// Friends</h3>
              <Link to="/friends" className="view-all">View All →</Link>
            </div>
            <div className="friends-preview">
              {friends.map(friend => (
                <Link
                  key={friend.id}
                  to={`/profile/${friend.username}`}
                  className="friend-preview-item"
                  aria-label={`${friend.username}'s profile`}
                >
                  <img src={friend.avatar} alt={`${friend.username} avatar`} loading="lazy" />
                  <span className="friend-name">{friend.username}</span>
                  <span
                    className={`friend-status ${friend.status}`}
                    aria-label={friend.status === 'online' ? 'Online' : 'Offline'}
                  />
                </Link>
              ))}
            </div>
          </section>
        </div>
      </div>
    </Layout>
  );
}
