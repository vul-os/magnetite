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

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Mock data — only used when VITE_USE_MOCKS === 'true'
const MOCK_STATS = {
  totalGames: 4,
  totalEarnings: 24580.50,
  totalPlayers: 12847,
  thisMonthRevenue: 4820.75,
};

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Raiders', status: 'Active', players: 8420, earnings: 12450.00, category: 'Action' },
  { id: 2, title: 'Galaxy Conquest', status: 'Active', players: 2941, earnings: 2970.50, category: 'Strategy' },
];

const MOCK_ACTIVITIES = [
  { id: 1, type: 'player', message: '128 new players joined Cosmic Raiders', time: '2 hours ago' },
  { id: 2, type: 'earnings', message: 'Earned $450.00 from Dungeon Realms', time: '6 hours ago' },
];

/* Design-token colours for recharts (must match CSS vars) */
const CHART_AMBER = '#f5a524';
const CHART_GRID  = '#23232e';
const CHART_TEXT  = '#6b6b78';
const CHART_BG    = '#14141d';

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
    case 'active':   return 'status-active';
    case 'pending':  return 'status-pending';
    case 'approved': return 'status-approved';
    case 'draft':    return 'status-draft';
    default:         return '';
  }
};

const getActivityIcon = (type) => {
  switch (type) {
    case 'player':   return '👥';
    case 'review':   return '⭐';
    case 'earnings': return '💰';
    default:         return '📌';
  }
};

export default function DeveloperDashboard() {
  const [stats, setStats]         = useState(USE_MOCKS ? MOCK_STATS : null);
  const [games, setGames]         = useState(USE_MOCKS ? MOCK_GAMES : []);
  const [revenueData, setRevenueData] = useState([]);
  const [activities]              = useState(USE_MOCKS ? MOCK_ACTIVITIES : []);
  const [loading, setLoading]     = useState(!USE_MOCKS);
  const [loadError, setLoadError] = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadData() {
      setLoading(true);
      setLoadError(null);
      try {
        const [dashData, gamesData] = await Promise.allSettled([
          api.developer.dashboard(),
          api.developer.games(),
        ]);

        if (cancelled) return;

        if (dashData.status === 'fulfilled') {
          const d = dashData.value?.data ?? dashData.value;
          setStats({
            totalGames: d?.total_games ?? 0,
            totalEarnings: Number(d?.total_earnings ?? 0),
            totalPlayers: Number(d?.total_players ?? 0),
            thisMonthRevenue: 0,
          });
          if (Array.isArray(d?.revenue_chart)) {
            setRevenueData(d.revenue_chart.map(p => ({
              date: p.date,
              revenue: Number(p.revenue ?? 0),
            })));
          }
        } else {
          throw dashData.reason;
        }

        if (gamesData.status === 'fulfilled') {
          const d = gamesData.value?.data ?? gamesData.value;
          const list = Array.isArray(d) ? d : (d?.games ?? []);
          setGames(list.map(g => ({
            id: g.id,
            title: g.title,
            status: g.status ?? 'active',
            players: Number(g.total_players ?? g.players ?? 0),
            earnings: Number(g.total_revenue ?? g.earnings ?? 0),
            category: g.category ?? '',
          })));
        }
      } catch (err) {
        if (!cancelled) setLoadError(err.message || 'Failed to load dashboard');
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadData();
    return () => { cancelled = true; };
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
            <span className="kicker">// RUST GAME DEVELOPER</span>
            <h1>Developer Dashboard</h1>
            <p>Track performance, manage Rust games, and withdraw earnings.</p>
          </div>
          <div className="header-actions">
            <Link to="/docs" className="btn btn-secondary">
              <span aria-hidden="true">📚</span> Documentation
            </Link>
            <Link to="/game-studio" className="btn btn-primary">
              <span aria-hidden="true">+</span> New Game
            </Link>
          </div>
        </header>

        {loadError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {loadError}
          </div>
        )}

        <section className="stats-section">
          <div className="stats-grid">
            <div className="stat-card">
              <div className="stat-icon-wrapper games-icon" aria-hidden="true">
                <span>🎮</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Games</span>
                <span className="stat-value">{loading ? '—' : (stats?.totalGames ?? 0)}</span>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon-wrapper players-icon" aria-hidden="true">
                <span>👥</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Players</span>
                <span className="stat-value">{loading ? '—' : (stats?.totalPlayers ?? 0).toLocaleString()}</span>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon-wrapper earnings-icon" aria-hidden="true">
                <span>💰</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">Total Earnings (USD)</span>
                <span className="stat-value amber-value">{loading ? '—' : `$${(stats?.totalEarnings ?? 0).toLocaleString()}`}</span>
              </div>
            </div>
            <div className="stat-card highlight">
              <div className="stat-icon-wrapper revenue-icon" aria-hidden="true">
                <span>📈</span>
              </div>
              <div className="stat-content">
                <span className="stat-label">This Month Revenue</span>
                <span className="stat-value amber-value">{loading ? '—' : `$${(stats?.thisMonthRevenue ?? 0).toLocaleString()}`}</span>
              </div>
            </div>
          </div>
        </section>

        <section className="main-content-grid">
          <div className="left-column">
            <div className="card revenue-chart-card">
              <div className="card-header">
                <div>
                  <span className="kicker" style={{ marginBottom: '0.25rem' }}>// 30-DAY VIEW</span>
                  <h2>Revenue Overview</h2>
                </div>
              </div>
              <div className="chart-container">
                <ResponsiveContainer width="100%" height={280}>
                  <AreaChart data={revenueData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                    <defs>
                      <linearGradient id="revenueGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%"  stopColor={CHART_AMBER} stopOpacity={0.3} />
                        <stop offset="95%" stopColor={CHART_AMBER} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke={CHART_GRID} vertical={false} />
                    <XAxis
                      dataKey="date"
                      stroke={CHART_TEXT}
                      tick={{ fill: CHART_TEXT, fontSize: 11, fontFamily: 'JetBrains Mono, monospace' }}
                      tickLine={false}
                      axisLine={false}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      stroke={CHART_TEXT}
                      tick={{ fill: CHART_TEXT, fontSize: 11, fontFamily: 'JetBrains Mono, monospace' }}
                      tickLine={false}
                      axisLine={false}
                      tickFormatter={(value) => `$${value}`}
                    />
                    <Tooltip
                      content={<CustomTooltip />}
                      contentStyle={{ background: CHART_BG }}
                    />
                    <Area
                      type="monotone"
                      dataKey="revenue"
                      stroke={CHART_AMBER}
                      strokeWidth={2}
                      fill="url(#revenueGradient)"
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>

            <div className="card games-table-card">
              <div className="card-header">
                <h2>My Rust Games</h2>
                <Link to="/game-studio" className="view-all-link">View All</Link>
              </div>
              {loading ? (
                <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>Loading games…</div>
              ) : games.length === 0 ? (
                <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>
                  <p>No games yet.</p>
                  <Link to="/game-studio" className="btn btn-primary" style={{ marginTop: '1rem', display: 'inline-block' }}>Create Your First Game</Link>
                </div>
              ) : (
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
                          <button className="action-btn edit" title="Edit" aria-label={`Edit ${game.title}`}>
                            ✏️
                          </button>
                          <button className="action-btn analytics" title="View Analytics" aria-label={`Analytics for ${game.title}`}>
                            📊
                          </button>
                          <button
                            className="action-btn delete"
                            title="Delete"
                            aria-label={`Delete ${game.title}`}
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
              )}
            </div>
          </div>

          <div className="right-column">
            <div className="card quick-actions-card">
              <h2>Quick Actions</h2>
              <div className="quick-actions">
                <Link to="/game-studio" className="quick-action-btn primary">
                  <span className="action-icon" aria-hidden="true">🎮</span>
                  <span className="action-text">
                    <strong>Create New Game</strong>
                    <small>Build and deploy in Rust</small>
                  </span>
                </Link>
                <Link to="/docs" className="quick-action-btn">
                  <span className="action-icon" aria-hidden="true">📚</span>
                  <span className="action-text">
                    <strong>View Documentation</strong>
                    <small>SDK, API refs &amp; tutorials</small>
                  </span>
                </Link>
                <Link to="/analytics" className="quick-action-btn">
                  <span className="action-icon" aria-hidden="true">📈</span>
                  <span className="action-text">
                    <strong>View Analytics</strong>
                    <small>Deep dive into your stats</small>
                  </span>
                </Link>
                <Link to="/wallet" className="quick-action-btn">
                  <span className="action-icon" aria-hidden="true">💳</span>
                  <span className="action-text">
                    <strong>Manage Wallet</strong>
                    <small>Add funds or request payout</small>
                  </span>
                </Link>
              </div>
            </div>

            <div className="card activity-card">
              <h2>Recent Activity</h2>
              <div className="activity-feed">
                {activities.map(activity => (
                  <div key={activity.id} className="activity-item">
                    <div className="activity-icon" aria-hidden="true">
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
