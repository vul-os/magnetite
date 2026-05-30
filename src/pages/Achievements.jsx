import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import AchievementCard from '../components/AchievementCard';
import Skeleton from '../components/skeletons/Skeleton';
import EmptyState from '../components/empty/EmptyState';
import Button from '../components/common/Button';
import { mockAchievements, recentUnlocks } from '../data/mockAchievements';
import { api } from '../api/client';
import './social.css';

const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

const TrophyIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d="M8 21h8" />
    <path d="M12 17v4" />
    <path d="M7 4H4a2 2 0 0 0-2 2v2c0 2.2 1.8 4 4 4h1" />
    <path d="M17 4h3a2 2 0 0 1 2 2v2c0 2.2-1.8 4-4 4h-1" />
    <path d="M7 4v8a5 5 0 0 0 10 0V4" />
  </svg>
);

const CATEGORIES = [
  { key: 'all',      label: 'All'       },
  { key: 'gameplay', label: 'Gameplay'  },
  { key: 'social',   label: 'Social'    },
  { key: 'economic', label: 'Economic'  },
];

export default function Achievements() {
  const [category, setCategory]         = useState('all');
  const [showUnlocked, setShowUnlocked] = useState(true);
  const [achievements, setAchievements] = useState(useMocks ? mockAchievements : []);
  const [recent, setRecent]             = useState(useMocks ? recentUnlocks : []);
  const [loading, setLoading]           = useState(!useMocks);

  useEffect(() => {
    if (useMocks) return;
    let cancelled = false;

    async function loadAchievements() {
      try {
        const me = await api.auth.me();
        const userId = me?.id || me?.user_id;
        if (!userId) {
          if (!cancelled) setLoading(false);
          return;
        }

        const data = await api.achievements.list(userId);
        if (!cancelled) {
          if (data) {
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
          setLoading(false);
        }
      } catch {
        if (!cancelled) {
          // Show empty state on API failure — do not silently inject mock data
          setLoading(false);
        }
      }
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
  const totalCount    = achievements.length;
  const progressPct   = totalCount > 0 ? Math.round((unlockedCount / totalCount) * 100) : 0;

  return (
    <Layout>
      <div className="achievements-page reveal">
        <header className="page-header reveal-1">
          <span className="kicker">// Trophy Room</span>
          <h1>Achievements</h1>
          {loading ? (
            <div style={{ marginTop: '0.5rem' }}>
              <Skeleton variant="text" width="180px" height="14px" />
            </div>
          ) : (
            <>
              <p className="achievement-summary" aria-live="polite">
                {unlockedCount}/{totalCount} unlocked &mdash; {progressPct}% complete
              </p>
              <div className="completion-bar" aria-hidden="true">
                <div
                  className="completion-fill"
                  style={{ width: `${progressPct}%` }}
                  role="progressbar"
                  aria-valuenow={progressPct}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${progressPct}% achievements unlocked`}
                />
              </div>
            </>
          )}
        </header>

        <div className="recent-unlocks reveal-2" aria-label="Recent achievements">
          <h3>// Recent Unlocks</h3>
          <div className="recent-grid">
            {loading ? (
              Array.from({ length: 3 }).map((_, i) => (
                <div key={i} className="recent-item">
                  <Skeleton variant="rect" width="40px" height="40px" />
                  <Skeleton variant="text" width="80%" height="12px" />
                  <Skeleton variant="text" width="60%" height="11px" />
                </div>
              ))
            ) : recent.map(unlock => (
              <div key={unlock.id} className="recent-item">
                <span className="recent-icon" aria-hidden="true">{unlock.icon}</span>
                <span className="recent-name">{unlock.name}</span>
                <span className="recent-date">{new Date(unlock.unlockedAt).toLocaleDateString()}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="achievements-controls reveal-3">
          <div className="category-filters" role="group" aria-label="Category filter">
            {CATEGORIES.map(cat => (
              <button
                key={cat.key}
                className={`filter-btn ${category === cat.key ? 'active' : ''}`}
                onClick={() => setCategory(cat.key)}
                aria-pressed={category === cat.key}
                disabled={loading}
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
                disabled={loading}
              />
              Show unlocked only
            </label>
          </div>
        </div>

        <div
          className="achievements-grid reveal-4"
          role="list"
          aria-label="Achievements"
          aria-busy={loading}
        >
          {loading ? (
            Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                role="listitem"
                style={{
                  background: 'var(--color-bg-card)',
                  border: '1px solid var(--color-border)',
                  borderRadius: 'var(--radius-lg)',
                  padding: '1.25rem',
                  display: 'flex',
                  alignItems: 'flex-start',
                  gap: '1rem',
                }}
              >
                <Skeleton variant="rect" width="44px" height="44px" />
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <Skeleton variant="text" width="65%" height="14px" />
                  <Skeleton variant="text" width="90%" height="12px" />
                  <Skeleton variant="text" width="80%" height="12px" />
                  <Skeleton variant="rect" width="100%" height="4px" />
                </div>
              </div>
            ))
          ) : filteredAchievements.length === 0 ? (
            <div role="listitem" style={{ gridColumn: '1 / -1' }}>
              <EmptyState
                icon={TrophyIcon}
                title="No achievements here"
                description={
                  category === 'all'
                    ? 'Play games to start earning achievements.'
                    : `No ${category} achievements found. Try a different category.`
                }
                action={
                  category !== 'all' ? (
                    <Button variant="secondary" onClick={() => setCategory('all')}>
                      Show All
                    </Button>
                  ) : null
                }
              />
            </div>
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
