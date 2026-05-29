import { useState } from 'react';
import Layout from '../components/Layout';
import AchievementCard from '../components/AchievementCard';
import { mockAchievements, recentUnlocks } from '../data/mockAchievements';

const CATEGORIES = [
  { key: 'all', label: 'All' },
  { key: 'gameplay', label: 'Gameplay' },
  { key: 'social', label: 'Social' },
  { key: 'economic', label: 'Economic' },
];

export default function Achievements() {
  const [category, setCategory] = useState('all');
  const [showUnlocked, setShowUnlocked] = useState(true);

  const filteredAchievements = mockAchievements.filter(achievement => {
    const matchesCategory = category === 'all' || achievement.category === category;
    const matchesUnlocked = showUnlocked || achievement.unlockedAt === null;
    return matchesCategory && matchesUnlocked;
  });

  const unlockedCount = mockAchievements.filter(a => a.unlockedAt !== null).length;
  const totalCount = mockAchievements.length;

  return (
    <Layout>
      <div className="achievements-page">
        <header className="page-header">
          <h1>Achievements</h1>
          <p className="achievement-summary">
            {unlockedCount} of {totalCount} achievements unlocked
          </p>
        </header>

        <div className="recent-unlocks">
          <h3>Recent Unlocks</h3>
          <div className="recent-grid">
            {recentUnlocks.map(unlock => (
              <div key={unlock.id} className="recent-item">
                <span className="recent-icon">{unlock.icon}</span>
                <span className="recent-name">{unlock.name}</span>
                <span className="recent-date">{new Date(unlock.unlockedAt).toLocaleDateString()}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="achievements-controls">
          <div className="category-filters">
            {CATEGORIES.map(cat => (
              <button
                key={cat.key}
                className={`filter-btn ${category === cat.key ? 'active' : ''}`}
                onClick={() => setCategory(cat.key)}
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

        <div className="achievements-grid">
          {filteredAchievements.length === 0 ? (
            <p className="empty-state">No achievements found</p>
          ) : (
            filteredAchievements.map(achievement => (
              <AchievementCard key={achievement.id} achievement={achievement} />
            ))
          )}
        </div>
      </div>
    </Layout>
  );
}