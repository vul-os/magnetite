import { useState, useEffect } from 'react';
import { Link, useParams } from 'react-router-dom';
import Layout from '../components/Layout';
import ProfileCard from '../components/ProfileCard';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './social.css';

const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

export default function Profile() {
  const { t } = useTranslation();
  const { username } = useParams();
  const [isFollowing, setIsFollowing] = useState(false);
  const [loading, setLoading]         = useState(true);
  const [error, setError]             = useState(null);
  const [user, setUser]               = useState(null);
  const [recentGames, setRecentGames] = useState([]);
  const [achievements, setAchievements] = useState([]);
  const [friends, setFriends]         = useState([]);

  useEffect(() => {
    let cancelled = false;

    async function loadProfile() {
      setLoading(true);
      setError(null);

      if (useMocks) {
        const { mockProfileUser, mockRecentGames, mockProfileAchievements, mockProfileFriends } =
          await import('../data/mockProfile');
        if (!cancelled) {
          setUser(mockProfileUser);
          setRecentGames(mockRecentGames);
          setAchievements(mockProfileAchievements);
          setFriends(mockProfileFriends);
          setLoading(false);
        }
        return;
      }

      try {
        const target = username || (await api.auth.me().then(me => me?.username).catch(() => null));
        if (!target) {
          if (!cancelled) { setLoading(false); setError('Not signed in'); }
          return;
        }

        const data = await api.profile.get(target);
        if (!cancelled && data) {
          const userData = data.user || data;
          setUser(userData);
          if (data.recent_games) setRecentGames(data.recent_games);
          if (data.achievements) setAchievements(data.achievements.slice(0, 3));
          if (data.friends) setFriends(data.friends.slice(0, 4));
        }
      } catch (err) {
        if (!cancelled) {
          setError(err.message || 'Failed to load profile');
          // No mock fallback — show the error state so the failure is visible
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadProfile();
    return () => { cancelled = true; };
  }, [username]);

  if (loading) {
    return (
      <Layout>
        <div className="loading-state" aria-live="polite" aria-busy="true" style={{ minHeight: '40vh', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '1rem' }}>
          <span className="spinner" aria-hidden="true" />
          <span>{t('account.loadingProfile')}</span>
        </div>
      </Layout>
    );
  }

  if (error && !user) {
    return (
      <Layout>
        <div className="error-state" role="alert" style={{ padding: '2rem', textAlign: 'center' }}>
          <p style={{ color: 'var(--color-error)' }}>{t('account.couldNotLoadProfile', { error })}</p>
        </div>
      </Layout>
    );
  }

  return (
    <Layout>
      <div className="profile-page reveal">
        {error && (
          <div className="profile-error-banner" role="status" aria-live="polite" style={{ padding: '0.5rem 1rem', background: 'var(--color-amber-soft)', color: 'var(--color-amber)', fontSize: '0.85rem', borderRadius: 'var(--radius-sm)', marginBottom: '1rem' }}>
            {t('account.showingCachedData')}
          </div>
        )}
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
          <section className="profile-section reveal-2" aria-labelledby="recent-games-heading">
            <div className="section-header">
              <h3 id="recent-games-heading">{t('account.recentGames')}</h3>
              <Link to="/leaderboard" className="view-all">{t('account.viewAll')}</Link>
            </div>
            <div className="recent-games-grid">
              {recentGames.map(game => (
                <div key={game.id} className="recent-game-card">
                  <img src={game.thumbnail} alt={game.title} loading="lazy" />
                  <div className="recent-game-info">
                    <h4>{game.title}</h4>
                    <p>{t('account.score', { score: game.score.toLocaleString() })}</p>
                    <p>{t('account.rank', { rank: game.rank })}</p>
                    <span className="played-date">
                      {new Date(game.playedAt).toLocaleDateString()}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="profile-section reveal-3" aria-labelledby="achievements-heading">
            <div className="section-header">
              <h3 id="achievements-heading">{t('account.achievements')}</h3>
              <Link to="/achievements" className="view-all">{t('account.viewAll')}</Link>
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

          <section className="profile-section reveal-4" aria-labelledby="friends-heading">
            <div className="section-header">
              <h3 id="friends-heading">{t('account.friends')}</h3>
              <Link to="/friends" className="view-all">{t('account.viewAll')}</Link>
            </div>
            <div className="friends-preview">
              {friends.map(friend => (
                <Link
                  key={friend.id}
                  to={`/profile/${friend.username}`}
                  className="friend-preview-item"
                  aria-label={t('account.friendProfileLabel', { username: friend.username })}
                >
                  <img src={friend.avatar} alt={t('account.friendAvatarAlt', { username: friend.username })} loading="lazy" />
                  <span className="friend-name">{friend.username}</span>
                  <span
                    className={`friend-status ${friend.status}`}
                    aria-label={friend.status === 'online' ? t('social.online') : t('social.offline')}
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
