import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import { api } from '../api/client';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer
} from 'recharts';
import './DeveloperDashboard.css';

const MOCK_STATS = {
  totalGames: 4,
  totalEarnings: 24580.50,
  totalPlayers: 12847,
  thisMonthRevenue: 4820.75,
};

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Raiders', status: 'Active', players: 8420, earnings: 12450.00, category: 'Action' },
  { id: 2, title: 'Galaxy Conquest', status: 'Active', players: 2941, earnings: 2970.50, category: 'Strategy' },
  { id: 3, title: 'Neon Drift', status: 'Draft', players: 0, earnings: 0, category: 'Racing' },
  { id: 4, title: 'Dungeon Realms', status: 'Pending', players: 1486, earnings: 9160.00, category: 'RPG' },
];

const generateRevenueData = () => {
  const data = [];
  const today = new Date();
  for (let i = 29; i >= 0; i--) {
    const date = new Date(today);
    date.setDate(date.getDate() - i);
    data.push({
      date: date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
      revenue: Math.floor(Math.random() * 500) + 100,
    });
  }
  return data;
};

const MOCK_ACTIVITIES = [
  { id: 1, type: 'player', message: '128 new players joined Cosmic Raiders', time: '2 hours ago' },
  { id: 2, type: 'review', message: 'New 5-star review on Galaxy Conquest', time: '4 hours ago' },
  { id: 3, type: 'earnings', message: 'Earned 450.00 USDC from Dungeon Realms', time: '6 hours ago' },
  { id: 4, type: 'player', message: '95 new players joined Dungeon Realms', time: '8 hours ago' },
  { id: 5, type: 'review', message: 'New 4-star review on Cosmic Raiders', time: '12 hours ago' },
];

const CustomTooltip = ({ active, payload, label }) => {
  if (active && payload && payload.length) {
    return (
      <div className="chart-tooltip">
        <p className="tooltip-label">{label}</p>
        <p className="tooltip-value">${payload[0].value.toLocaleString()}</p>
      </div>
    );
  }
  return null;
};

const getStatusClass = (status) => {
  switch (status.toLowerCase()) {
    case 'active': return 'status-active';
    case 'pending': return 'status-pending';
    case 'approved': return 'status-approved';
    case 'draft': return 'status-draft';
    default: return '';
  }
};

const getActivityIcon = (type) => {
  switch (type) {
    case 'player': return '👥';
    case 'review': return '⭐';
    case 'earnings': return '💰';
    default: return '📌';
  }
};

export default function DeveloperDashboard() {
  const [stats, setStats] = useState(MOCK_STATS);
  const [games, setGames] = useState(MOCK_GAMES);
  const [revenueData] = useState(generateRevenueData);
  const [activities] = useState(MOCK_ACTIVITIES);

  useEffect(() => {
    async function loadData() {
      try {
        const [gamesData, walletData] = await Promise.allSettled([
          api.games.list(),
          api.wallet.balance(),
        ]);
        if (gamesData.status === 'fulfilled') {
          setGames(gamesData.value.games || MOCK_GAMES);
        }
        if (walletData.status === 'fulfilled') {
          setStats(prev => ({ ...prev, totalEarnings: walletData.value.balance || prev.totalEarnings }));
        }
      } catch {
        console.log('Using mock data');
      }
    }
    loadData();
  }, []);

  const handleDeleteGame = (gameId) => {
    if (window.confirm('Are you sure you want to delete this game?')) {
      setGames(prev => prev.filter(g => g.id !== gameId));
    }
  };

  return (
    <Layout>
      <div className="developer-dashboard">
        <header className="dashboard-header">
          <div className="header-content">
            <h1>Developer Dashboard</h1>
            <p>Welcome back! Here's what's happening with your games.</p>
          </div>
          <div className="header-actions">
            <Link to="/docs" className="btn btn-secondary">
              <span>📚</span> View Documentation
            </Link>
            <Link to="/game-studio" className="btn btn-primary">
              <span>+</span> Create New Game
            </Link>
          </div>
        </header>

        <section className="stats-section">
          <div className="stats-grid">
            <div className="stat-card">
              <div className="stat-icon-wrapper games-icon">
                <span>🎮</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Games</span>
                <span className="stat-value">{stats.totalGames}</span>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon-wrapper players-icon">
                <span>👥</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Players</span>
                <span className="stat-value">{stats.totalPlayers.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon-wrapper earnings-icon">
                <span>💰</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Earnings (USDC)</span>
                <span className="stat-value">${stats.totalEarnings.toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card highlight">
              <div className="stat-icon-wrapper revenue-icon">
                <span>📈</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">This Month Revenue</span>
                <span className="stat-value">${stats.thisMonthRevenue.toLocaleString()}</span>
              </div>
            </div>
          </div>
        </section>

        <section className="main-content-grid">
          <div className="left-column">
            <div className="card revenue-chart-card">
              <div className="card-header">
                <h2>Revenue Overview</h2>
                <span className="card-subtitle">30-day view</span>
              </div>
              <div className="chart-container">
                <ResponsiveContainer width="100%" height={280}>
                  <AreaChart data={revenueData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                    <defs>
                      <linearGradient id="revenueGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="#f59e0b" stopOpacity={0.3} />
                        <stop offset="95%" stopColor="#f59e0b" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="#27272a" vertical={false} />
                    <XAxis
                      dataKey="date"
                      stroke="#71717a"
                      fontSize={12}
                      tickLine={false}
                      axisLine={false}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      stroke="#71717a"
                      fontSize={12}
                      tickLine={false}
                      axisLine={false}
                      tickFormatter={(value) => `$${value}`}
                    />
                    <Tooltip content={<CustomTooltip />} />
                    <Area
                      type="monotone"
                      dataKey="revenue"
                      stroke="#f59e0b"
                      strokeWidth={2}
                      fill="url(#revenueGradient)"
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>

            <div className="card games-table-card">
              <div className="card-header">
                <h2>My Games</h2>
                <Link to="/game-studio" className="view-all-link">View All</Link>
              </div>
              <table className="games-table">
                <thead>
                  <tr>
                    <th>Game</th>
                    <th>Status</th>
                    <th>Players</th>
                    <th>Earnings</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {games.map(game => (
                    <tr key={game.id}>
                      <td>
                        <div className="game-cell">
                          <span className="game-title">{game.title}</span>
                          <span className="game-category">{game.category}</span>
                        </div>
                      </td>
                      <td>
                        <span className={`status-badge ${getStatusClass(game.status)}`}>
                          {game.status}
                        </span>
                      </td>
                      <td className="players-cell">{game.players.toLocaleString()}</td>
                      <td className="earnings-cell">${game.earnings.toLocaleString()}</td>
                      <td>
                        <div className="actions-cell">
                          <button className="action-btn edit" title="Edit">
                            ✏️
                          </button>
                          <button className="action-btn analytics" title="View Analytics">
                            📊
                          </button>
                          <button
                            className="action-btn delete"
                            title="Delete"
                            onClick={() => handleDeleteGame(game.id)}
                          >
                            🗑️
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <div className="right-column">
            <div className="card quick-actions-card">
              <h2>Quick Actions</h2>
              <div className="quick-actions">
                <Link to="/game-studio" className="quick-action-btn primary">
                  <span className="action-icon">🎮</span>
                  <span className="action-text">
                    <strong>Create New Game</strong>
                    <small>Build and deploy your next hit</small>
                  </span>
                </Link>
                <Link to="/docs" className="quick-action-btn">
                  <span className="action-icon">📚</span>
                  <span className="action-text">
                    <strong>View Documentation</strong>
                    <small>API refs and tutorials</small>
                  </span>
                </Link>
                <Link to="/analytics" className="quick-action-btn">
                  <span className="action-icon">📈</span>
                  <span className="action-text">
                    <strong>View Analytics</strong>
                    <small>Deep dive into your stats</small>
                  </span>
                </Link>
                <Link to="/wallet" className="quick-action-btn">
                  <span className="action-icon">💳</span>
                  <span className="action-text">
                    <strong>Manage Wallet</strong>
                    <small>Withdraw or deposit funds</small>
                  </span>
                </Link>
              </div>
            </div>

            <div className="card activity-card">
              <h2>Recent Activity</h2>
              <div className="activity-feed">
                {activities.map(activity => (
                  <div key={activity.id} className="activity-item">
                    <div className="activity-icon">
                      {getActivityIcon(activity.type)}
                    </div>
                    <div className="activity-content">
                      <p className="activity-message">{activity.message}</p>
                      <span className="activity-time">{activity.time}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </section>
      </div>
    </Layout>
  );
}
