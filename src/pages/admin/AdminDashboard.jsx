import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import './admin.css';

const MOCK_STATS = {
  totalUsers:     12847,
  totalGames:     342,
  totalRevenue:   245890.50,
  pendingPayouts: 12450.00,
  activeSessions: 1842,
  newUsersToday:  127,
};

const MOCK_RECENT_ACTIVITY = [
  { id: 1, type: 'user_register',    message: 'New user registered: CryptoGamer42',                 time: '2 min ago'  },
  { id: 2, type: 'game_submitted',   message: 'Game submitted: Neon Drift by StarForge Studios',    time: '15 min ago' },
  { id: 3, type: 'payout_request',   message: 'Payout request: $500.00 from PixelMaster',           time: '32 min ago' },
  { id: 4, type: 'game_approved',    message: 'Game approved: Galaxy Conquest',                      time: '1h ago'     },
  { id: 5, type: 'user_banned',      message: 'User banned: Cheater123 for match manipulation',      time: '2h ago'     },
  { id: 6, type: 'large_transaction',message: 'Large transaction: $2,500 in game fees',              time: '3h ago'     },
];

const MOCK_QUICK_ACTIONS = [
  { id: 'users',    label: 'Manage Users',     icon: '👥', link: '/admin/users'    },
  { id: 'games',    label: 'Review Games',     icon: '⬡',  link: '/admin/games'    },
  { id: 'finance',  label: 'View Finance',     icon: '$',  link: '/admin/finance'  },
  { id: 'settings', label: 'Platform Settings',icon: '⚙',  link: '/admin/settings' },
];

const STATS_CONFIG = [
  { key: 'totalUsers',     label: 'Total Users',     icon: '👥', value: MOCK_STATS.totalUsers.toLocaleString(),             warning: false },
  { key: 'totalGames',     label: 'Total Games',     icon: '⬡',  value: MOCK_STATS.totalGames,                              warning: false },
  { key: 'totalRevenue',   label: 'Total Revenue',   icon: '$',  value: `$${MOCK_STATS.totalRevenue.toLocaleString()}`,     warning: false },
  { key: 'pendingPayouts', label: 'Pending Payouts', icon: '⏳', value: `$${MOCK_STATS.pendingPayouts.toLocaleString()}`,   warning: true  },
  { key: 'activeSessions', label: 'Active Sessions', icon: '◉',  value: MOCK_STATS.activeSessions.toLocaleString(),        warning: false },
  { key: 'newUsersToday',  label: 'New Today',       icon: '↑',  value: MOCK_STATS.newUsersToday,                          warning: false },
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

          <div className="admin-stats-grid reveal-2">
            {STATS_CONFIG.map(stat => (
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

          <div className="admin-dashboard-grid reveal-3">
            <section className="admin-card activity-feed">
              <div className="admin-card-header">
                <h2 className="admin-card-title">// Recent Activity</h2>
              </div>
              <ul className="admin-activity-list" aria-label="Recent platform activity">
                {MOCK_RECENT_ACTIVITY.map(activity => (
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
            </section>

            <section className="admin-card quick-actions">
              <div className="admin-card-header">
                <h2 className="admin-card-title">// Quick Actions</h2>
              </div>
              <div className="admin-actions-grid">
                {MOCK_QUICK_ACTIONS.map(action => (
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
