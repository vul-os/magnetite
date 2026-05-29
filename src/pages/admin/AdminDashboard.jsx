import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';

const MOCK_STATS = {
  totalUsers: 12847,
  totalGames: 342,
  totalRevenue: 245890.50,
  pendingPayouts: 12450.00,
  activeSessions: 1842,
  newUsersToday: 127,
};

const MOCK_RECENT_ACTIVITY = [
  { id: 1, type: 'user_register', message: 'New user registered: CryptoGamer42', time: '2 minutes ago' },
  { id: 2, type: 'game_submitted', message: 'Game submitted: Neon Drift by StarForge Studios', time: '15 minutes ago' },
  { id: 3, type: 'payout_request', message: 'Payout request: $500.00 from PixelMaster', time: '32 minutes ago' },
  { id: 4, type: 'game_approved', message: 'Game approved: Galaxy Conquest', time: '1 hour ago' },
  { id: 5, type: 'user_banned', message: 'User banned: Cheater123 for match manipulation', time: '2 hours ago' },
  { id: 6, type: 'large_transaction', message: 'Large transaction: $2,500 in game fees', time: '3 hours ago' },
];

const MOCK_QUICK_ACTIONS = [
  { id: 'users', label: 'Manage Users', icon: '👥', link: '/admin/users', color: '#6366f1' },
  { id: 'games', label: 'Review Games', icon: '🎮', link: '/admin/games', color: '#10b981' },
  { id: 'finance', label: 'View Finance', icon: '💰', link: '/admin/finance', color: '#f59e0b' },
  { id: 'settings', label: 'Platform Settings', icon: '⚙️', link: '/admin/settings', color: '#64748b' },
];

export default function AdminDashboard() {
  const getActivityIcon = (type) => {
    const icons = {
      user_register: '👤',
      game_submitted: '🎮',
      payout_request: '💸',
      game_approved: '✅',
      user_banned: '🚫',
      large_transaction: '💰',
    };
    return icons[type] || '📌';
  };

  const getActivityClass = (type) => {
    const classes = {
      user_register: 'activity-new',
      game_submitted: 'activity-game',
      payout_request: 'activity-payout',
      game_approved: 'activity-success',
      user_banned: 'activity-danger',
      large_transaction: 'activity-money',
    };
    return classes[type] || '';
  };

  return (
    <Layout>
      <div className="admin-dashboard">
        <AdminSidebar />
        <main className="admin-main">
          <header className="admin-header">
            <h1>Admin Dashboard</h1>
            <p>Overview of the Magnetite platform</p>
          </header>

          <div className="stats-grid">
            <div className="stat-card">
              <span className="stat-icon">👥</span>
              <div className="stat-info">
                <span className="stat-label">Total Users</span>
                <span className="stat-value">{MOCK_STATS.totalUsers.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon">🎮</span>
              <div className="stat-info">
                <span className="stat-label">Total Games</span>
                <span className="stat-value">{MOCK_STATS.totalGames}</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon">💰</span>
              <div className="stat-info">
                <span className="stat-label">Total Revenue</span>
                <span className="stat-value">${MOCK_STATS.totalRevenue.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card warning">
              <span className="stat-icon">⏳</span>
              <div className="stat-info">
                <span className="stat-label">Pending Payouts</span>
                <span className="stat-value">${MOCK_STATS.pendingPayouts.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon">🟢</span>
              <div className="stat-info">
                <span className="stat-label">Active Sessions</span>
                <span className="stat-value">{MOCK_STATS.activeSessions.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card">
              <span className="stat-icon">📈</span>
              <div className="stat-info">
                <span className="stat-label">New Users Today</span>
                <span className="stat-value">{MOCK_STATS.newUsersToday}</span>
              </div>
            </div>
          </div>

          <div className="dashboard-grid">
            <section className="dashboard-card activity-feed">
              <h2>Recent Activity</h2>
              <div className="activity-list">
                {MOCK_RECENT_ACTIVITY.map(activity => (
                  <div key={activity.id} className={`activity-item ${getActivityClass(activity.type)}`}>
                    <span className="activity-icon">{getActivityIcon(activity.type)}</span>
                    <div className="activity-content">
                      <span className="activity-message">{activity.message}</span>
                      <span className="activity-time">{activity.time}</span>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            <section className="dashboard-card quick-actions">
              <h2>Quick Actions</h2>
              <div className="actions-grid">
                {MOCK_QUICK_ACTIONS.map(action => (
                  <Link
                    key={action.id}
                    to={action.link}
                    className="action-card"
                    style={{ '--action-color': action.color }}
                  >
                    <span className="action-icon">{action.icon}</span>
                    <span className="action-label">{action.label}</span>
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
