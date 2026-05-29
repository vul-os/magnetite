import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import AchievementCard from '../components/AchievementCard';
import { mockAchievements, recentUnlocks } from '../data/mockAchievements';
import { api } from '../api/client';
import './social.css';

const CATEGORIES = [
  { key: 'all', label: 'All' },
  { key: 'gameplay', label: 'Gameplay' },
  { key: 'social', label: 'Social' },
  { key: 'economic', label: 'Economic' },
];

export default function Achievements() {
  const [category, setCategory] = useState('all');
  const [showUnlocked, setShowUnlocked] = useState(true);
  const [achievements, setAchievements] = useState(mockAchievements);
  const [recent, setRecent] = useState(recentUnlocks);

  useEffect(() => {
    let cancelled = false;

    async function loadAchievements() {
      try {
        // Try to get user info first for the userId param
        const me = await api.auth.me();
        const userId = me?.id || me?.user_id;
        if (!userId) return;

        const data = await api.achievements.list(userId);
        if (!cancelled && data) {
          const list = Array.isArray(data) ? data : (data?.achievements ?? null);
          if (list && list.length > 0) {
            setAchievements(list);
            const recentList = list
              .filter(a => a.unlockedAt)
              .sort((a, b) => new Date(b.unlockedAt) - new Date(a.unlockedAt))
              .slice(0, 3);
            if (recentList.length > 0) setRecent(recentList);
          }
        }
      } catch { /* use mock data */ }
    }

    loadAchievements();
    return () => { cancelled = true; };
  }, []);

  const filteredAchievements = achievements.filter(achievement => {
    const matchesCategory = category === 'all' || achievement.category === category;
    const matchesUnlocked = showUnlocked || achievement.unlockedAt === null;
    return matchesCategory && matchesUnlocked;
  });

  const unlockedCount = achievements.filter(a => a.unlockedAt !== null).length;
  const totalCount = achievements.length;

  return (
    <Layout>
      <div className="achievements-page">
        <header className="page-header">
          <h1>Achievements</h1>
          <p className="achievement-summary">
            {unlockedCount}/{totalCount} unlocked
          </p>
        </header>

        <div className="recent-unlocks">
          <h3>// RECENT UNLOCKS</h3>
          <div className="recent-grid">
            {recent.map(unlock => (
              <div key={unlock.id} className="recent-item">
                <span className="recent-icon" aria-hidden="true">{unlock.icon}</span>
                <span className="recent-name">{unlock.name}</span>
                <span className="recent-date">{new Date(unlock.unlockedAt).toLocaleDateString()}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="achievements-controls">
          <div className="category-filters" role="group" aria-label="Category filter">
            {CATEGORIES.map(cat => (
              <button
                key={cat.key}
                className={`filter-btn ${category === cat.key ? 'active' : ''}`}
                onClick={() => setCategory(cat.key)}
                aria-pressed={category === cat.key}
              >
                {cat.label}
              </button>
            ))}
          </div>

          <div className="toggle-wrapper">
            <label>
              <input
                type="checkbox"
                checked={showUnlocked}
                onChange={(e) => setShowUnlocked(e.target.checked)}
              />
              Show unlocked only
            </label>
          </div>
        </div>

        <div className="achievements-grid" role="list" aria-label="Achievements">
          {filteredAchievements.length === 0 ? (
            <p className="empty-state-inline">No achievements found</p>
          ) : (
            filteredAchievements.map(achievement => (
              <div key={achievement.id} role="listitem">
                <AchievementCard achievement={achievement} />
              </div>
            ))
          )}
        </div>
      </div>
    </Layout>
  );
}
