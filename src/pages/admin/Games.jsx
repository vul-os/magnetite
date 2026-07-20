import { useState, useMemo, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
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

/* Mock data — only used when VITE_USE_MOCKS=true */
const MOCK_GAMES = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 1, title: 'Cosmic Raiders',  developer: 'StarForge Studios', status: 'active',   featured: true,  players: 8420,  revenue: 12450.00, submittedAt: '2024-01-10' },
      { id: 2, title: 'Galaxy Conquest', developer: 'PixelMaster',       status: 'active',   featured: false, players: 2941,  revenue: 2970.50,  submittedAt: '2024-01-15' },
      { id: 3, title: 'Neon Drift',      developer: 'StarForge Studios', status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-18' },
      { id: 4, title: 'Dungeon Realms',  developer: 'IndieDev_Mike',     status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-19' },
    ]
  : null;

const STATUS_OPTIONS = [
  { value: 'all',      label: 'All Status' },
  { value: 'pending',  label: 'Pending'    },
  { value: 'active',   label: 'Active'     },
  { value: 'rejected', label: 'Rejected'   },
];

function normaliseGame(g) {
  return {
    id:          g.id,
    title:       g.title,
    developer:   g.developer_username ?? g.developer ?? 'Unknown',
    status:      g.status ?? 'pending',
    featured:    g.featured_at != null,
    players:     g.players ?? 0,
    revenue:     parseFloat(g.fee_per_session ?? g.revenue ?? 0),
    submittedAt: g.created_at ? g.created_at.split('T')[0] : '',
  };
}

export default function Games() {
  const [games, setGames]             = useState(MOCK_GAMES ? MOCK_GAMES.map(g => ({ ...g })) : []);
  const [loading, setLoading]         = useState(!MOCK_GAMES);
  const [error, setError]             = useState(null);
  const [statusFilter, setStatusFilter]       = useState('all');
  const [developerFilter, setDeveloperFilter] = useState('');
  const [currentPage, setCurrentPage]         = useState(1);
  const [actionLoading, setActionLoading]     = useState(null);
  const [actionError, setActionError]         = useState(null);
  const perPage = 10;

  const fetchGames = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    setLoading(true);
    setError(null);
    try {
      const res = await authFetch('/api/admin/games?limit=200');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = await res.json();
      const raw = json.data ?? json ?? [];
      setGames(Array.isArray(raw) ? raw.map(normaliseGame) : []);
    } catch (err) {
      setError(err.message || 'Failed to load games');
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch games from the admin API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchGames(); }, [fetchGames]);

  const developers = useMemo(
    () => [...new Set(games.map(g => g.developer))].sort(),
    [games]
  );

  const filteredGames = useMemo(() => {
    let list = [...games];
    if (statusFilter !== 'all')  list = list.filter(g => g.status    === statusFilter);
    if (developerFilter)         list = list.filter(g => g.developer === developerFilter);
    return list;
  }, [games, statusFilter, developerFilter]);

  const paginatedGames = useMemo(() => {
    const start = (currentPage - 1) * perPage;
    return filteredGames.slice(start, start + perPage);
  }, [filteredGames, currentPage, perPage]);

  const pendingCount = games.filter(g => g.status === 'pending').length;

  const handleAction = async (action, gameId) => {
    setActionLoading(gameId);
    setActionError(null);
    try {
      let res;
      if (action === 'approve') {
        res = await authFetch(`/api/admin/games/${gameId}/approve`, {
          method: 'PUT',
          body: JSON.stringify({ approved: true }),
        });
      } else if (action === 'reject') {
        res = await authFetch(`/api/admin/games/${gameId}/approve`, {
          method: 'PUT',
          body: JSON.stringify({ approved: false }),
        });
      } else if (action === 'feature') {
        res = await authFetch(`/api/admin/games/${gameId}/feature`, {
          method: 'PUT',
          body: JSON.stringify({ featured: true }),
        });
      } else if (action === 'unfeature') {
        res = await authFetch(`/api/admin/games/${gameId}/feature`, {
          method: 'PUT',
          body: JSON.stringify({ featured: false }),
        });
      }
      if (res && !res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || `Action failed (HTTP ${res.status})`);
      }
      /* optimistic update */
      setGames(prev => prev.map(g => {
        if (g.id !== gameId && String(g.id) !== String(gameId)) return g;
        if (action === 'approve')   return { ...g, status: 'active'    };
        if (action === 'reject')    return { ...g, status: 'rejected'  };
        if (action === 'feature')   return { ...g, featured: true      };
        if (action === 'unfeature') return { ...g, featured: false     };
        return g;
      }));
    } catch (err) {
      setActionError(err.message);
    } finally {
      setActionLoading(null);
    }
  };

  const statusBadgeClass = (status) => ({
    active:   'status-badge active',
    pending:  'status-badge pending',
    rejected: 'status-badge rejected',
    approved: 'status-badge active',
  }[status] || 'status-badge');

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Platform Control</span>
              <h1>Game Management</h1>
              <p>Review and manage Rust games</p>
            </div>
            {pendingCount > 0 && (
              <div className="pending-badge" role="status">
                {pendingCount} game{pendingCount !== 1 ? 's' : ''} pending review
              </div>
            )}
          </header>

          {error && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {error}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={fetchGames}>
                Retry
              </button>
            </div>
          )}

          {actionError && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {actionError}
            </div>
          )}

          <div className="admin-toolbar">
            <select
              value={statusFilter}
              onChange={(e) => { setStatusFilter(e.target.value); setCurrentPage(1); }}
              aria-label="Filter by status"
            >
              {STATUS_OPTIONS.map(opt => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
            <select
              value={developerFilter}
              onChange={(e) => { setDeveloperFilter(e.target.value); setCurrentPage(1); }}
              aria-label="Filter by developer"
            >
              <option value="">All Developers</option>
              {developers.map(dev => (
                <option key={dev} value={dev}>{dev}</option>
              ))}
            </select>
          </div>

          <div className="admin-table-container">
            {loading ? (
              <div className="admin-loading" aria-busy="true" style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>
                <span className="spinner" aria-hidden="true" /> Loading games&hellip;
              </div>
            ) : filteredGames.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                No games found
              </div>
            ) : (
              <table className="admin-table" aria-label="Games">
                <thead>
                  <tr>
                    <th>Game</th>
                    <th>Developer</th>
                    <th>Status</th>
                    <th>Featured</th>
                    <th>Players</th>
                    <th>Revenue</th>
                    <th>Submitted</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {paginatedGames.map(game => (
                    <tr key={game.id}>
                      <td>
                        <div className="game-cell">
                          <span className="game-icon" aria-hidden="true">⬡</span>
                          <span className="game-title">{game.title}</span>
                        </div>
                      </td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                        {game.developer}
                      </td>
                      <td>
                        <span className={statusBadgeClass(game.status)}>{game.status}</span>
                      </td>
                      <td>
                        {game.featured
                          ? <span className="featured-badge">★ Featured</span>
                          : <span className="not-featured">—</span>
                        }
                      </td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)' }}>
                        {game.players.toLocaleString()}
                      </td>
                      <td className="amount-cell">
                        ${game.revenue.toLocaleString()}
                      </td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-2xs)', color: 'var(--color-text-muted)' }}>
                        {game.submittedAt}
                      </td>
                      <td>
                        <div className="action-buttons">
                          {(game.status === 'pending') && (
                            <>
                              <Button
                                variant="primary"
                                size="sm"
                                loading={actionLoading === game.id}
                                onClick={() => handleAction('approve', game.id)}
                              >
                                Approve
                              </Button>
                              <Button
                                variant="danger"
                                size="sm"
                                loading={actionLoading === game.id}
                                onClick={() => handleAction('reject', game.id)}
                              >
                                Reject
                              </Button>
                            </>
                          )}
                          {(game.status === 'active' || game.status === 'approved') && (
                            <Button
                              variant={game.featured ? 'secondary' : 'primary'}
                              size="sm"
                              loading={actionLoading === game.id}
                              onClick={() => handleAction(game.featured ? 'unfeature' : 'feature', game.id)}
                            >
                              {game.featured ? 'Unfeature' : 'Feature'}
                            </Button>
                          )}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>

          <Pagination
            total={filteredGames.length}
            perPage={perPage}
            currentPage={currentPage}
            onPageChange={setCurrentPage}
          />
        </main>
      </div>
    </Layout>
  );
}
