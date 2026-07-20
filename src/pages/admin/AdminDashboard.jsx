import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import './admin.css';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

function authFetch(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  return fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });
}

const MOCK_STATS = import.meta.env.VITE_USE_MOCKS === 'true'
  ? {
      totalUsers: 12847,
      totalGames: 342,
      totalRevenue: 245890.5,
      pendingPayouts: 12450.0,
      activeSessions: 1842,
      newUsersToday: 127,
    }
  : null;

const MOCK_RECENT_ACTIVITY = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 1, type: 'user_register',     message: 'New user registered: CryptoGamer42',              time: '2 min ago'  },
      { id: 2, type: 'game_submitted',    message: 'Game submitted: Neon Drift by StarForge Studios',  time: '15 min ago' },
      { id: 3, type: 'payout_request',    message: 'Payout request: $500.00 from PixelMaster',         time: '32 min ago' },
      { id: 4, type: 'game_approved',     message: 'Game approved: Galaxy Conquest',                   time: '1h ago'     },
      { id: 5, type: 'user_banned',       message: 'User banned: Cheater123 for match manipulation',   time: '2h ago'     },
      { id: 6, type: 'large_transaction', message: 'Large transaction: $2,500 in game fees',           time: '3h ago'     },
    ]
  : null;

const QUICK_ACTIONS = [
  { id: 'users',    label: 'Manage Users',      icon: '👥', link: '/admin/users'    },
  { id: 'games',    label: 'Review Games',      icon: '⬡',  link: '/admin/games'    },
  { id: 'finance',  label: 'View Finance',      icon: '$',  link: '/admin/finance'  },
  { id: 'settings', label: 'Platform Settings', icon: '⚙',  link: '/admin/settings' },
];

function getActivityClass(type) {
  const classes = {
    user_register:     'activity-new',
    game_submitted:    'activity-game',
    payout_request:    'activity-payout',
    game_approved:     'activity-success',
    user_banned:       'activity-danger',
    large_transaction: 'activity-money',
  };
  return classes[type] || '';
}

function getActivityEmoji(type) {
  const icons = {
    user_register:     '👤',
    game_submitted:    '⬡',
    payout_request:    '💸',
    game_approved:     '✓',
    user_banned:       '⊘',
    large_transaction: '$',
  };
  return icons[type] || '·';
}

export default function AdminDashboard() {
  const [stats, setStats]     = useState(MOCK_STATS);
  const [metrics, setMetrics] = useState(null);
  const [loading, setLoading] = useState(!MOCK_STATS);
  const [error, setError]     = useState(null);

  useEffect(() => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const [overviewRes, metricsRes] = await Promise.all([
          authFetch('/api/admin/analytics/overview'),
          authFetch('/api/admin/metrics'),
        ]);

        if (overviewRes.ok) {
          const data = await overviewRes.json();
          setStats({
            totalUsers:     data.total_users    ?? 0,
            totalGames:     data.total_games    ?? 0,
            totalRevenue:   parseFloat(data.total_revenue ?? 0),
            pendingPayouts: parseFloat(data.pending_payouts_value ?? 0),
            activeSessions: data.active_sessions ?? 0,
            newUsersToday:  data.new_users_today ?? 0,
          });
        }

        if (metricsRes.ok) {
          setMetrics(await metricsRes.json());
        }
      } catch (err) {
        setError(err.message || 'Failed to load dashboard');
      } finally {
        setLoading(false);
      }
    }

    load();
  }, []);

  const statsConfig = stats
    ? [
        { key: 'totalUsers',     label: 'Total Users',     icon: '👥', value: stats.totalUsers.toLocaleString(),                    warning: false },
        { key: 'totalGames',     label: 'Total Games',     icon: '⬡',  value: stats.totalGames,                                     warning: false },
        { key: 'totalRevenue',   label: 'Total Revenue',   icon: '$',  value: `$${stats.totalRevenue.toLocaleString()}`,             warning: false },
        { key: 'pendingPayouts', label: 'Pending Payouts', icon: '⏳', value: `$${stats.pendingPayouts.toLocaleString()}`,           warning: true  },
        { key: 'activeSessions', label: 'Active Sessions', icon: '◉',  value: (metrics?.total_users ?? stats.activeSessions).toLocaleString(), warning: false },
        { key: 'newUsersToday',  label: 'New Today',       icon: '↑',  value: stats.newUsersToday,                                  warning: false },
      ]
    : [];

  const recentActivity = MOCK_RECENT_ACTIVITY || [];

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Control Panel</span>
              <h1>Admin Dashboard</h1>
              <p>Platform overview &mdash; Magnetite</p>
            </div>
          </header>

          {error && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {error}
              <button
                className="settings-action-btn"
                style={{ marginLeft: '1rem' }}
                onClick={() => window.location.reload()}
              >
                Retry
              </button>
            </div>
          )}

          {loading ? (
            <div className="admin-stats-grid reveal-2">
              {Array.from({ length: 6 }).map((_, i) => (
                <div key={i} className="admin-stat-card skeleton-card" aria-busy="true">
                  <div className="skeleton skeleton-icon" />
                  <div className="admin-stat-info">
                    <div className="skeleton skeleton-label" />
                    <div className="skeleton skeleton-value" />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="admin-stats-grid reveal-2">
              {statsConfig.map(stat => (
                <div
                  key={stat.key}
                  className={`admin-stat-card${stat.warning ? ' warning' : ''}`}
                  aria-label={`${stat.label}: ${stat.value}`}
                >
                  <div className="admin-stat-icon" aria-hidden="true">{stat.icon}</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">{stat.label}</span>
                    <span className="admin-stat-value">{stat.value}</span>
                  </div>
                </div>
              ))}
            </div>
          )}

          <div className="admin-dashboard-grid reveal-3">
            <section className="admin-card activity-feed">
              <div className="admin-card-header">
                <h2 className="admin-card-title">// Recent Activity</h2>
              </div>
              {recentActivity.length === 0 ? (
                <p style={{ padding: '1rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                  No recent activity
                </p>
              ) : (
                <ul className="admin-activity-list" aria-label="Recent platform activity">
                  {recentActivity.map(activity => (
                    <li
                      key={activity.id}
                      className={`admin-activity-item ${getActivityClass(activity.type)}`}
                    >
                      <div className="admin-activity-icon" aria-hidden="true">
                        {getActivityEmoji(activity.type)}
                      </div>
                      <div className="admin-activity-content">
                        <p className="admin-activity-message">{activity.message}</p>
                        <span className="admin-activity-time">{activity.time}</span>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </section>

            <section className="admin-card quick-actions">
              <div className="admin-card-header">
                <h2 className="admin-card-title">// Quick Actions</h2>
              </div>
              <div className="admin-actions-grid">
                {QUICK_ACTIONS.map(action => (
                  <Link
                    key={action.id}
                    to={action.link}
                    className="admin-action-card"
                    aria-label={action.label}
                  >
                    <span className="admin-action-icon" aria-hidden="true">{action.icon}</span>
                    <span className="admin-action-label">{action.label}</span>
                  </Link>
                ))}
              </div>
            </section>
          </div>
        </main>
      </div>
    </Layout>
  );
}
