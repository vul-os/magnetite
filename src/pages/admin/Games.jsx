import { useState, useMemo } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
import './admin.css';

const MOCK_GAMES = [
  { id: 1,  title: 'Cosmic Raiders',    developer: 'StarForge Studios', status: 'active',   featured: true,  players: 8420,  revenue: 12450.00, submittedAt: '2024-01-10' },
  { id: 2,  title: 'Galaxy Conquest',   developer: 'PixelMaster',       status: 'active',   featured: false, players: 2941,  revenue: 2970.50,  submittedAt: '2024-01-15' },
  { id: 3,  title: 'Neon Drift',        developer: 'StarForge Studios', status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-18' },
  { id: 4,  title: 'Dungeon Realms',    developer: 'IndieDev_Mike',     status: 'active',   featured: true,  players: 1486,  revenue: 9160.00,  submittedAt: '2024-02-01' },
  { id: 5,  title: 'Space Battle',      developer: 'BlockchainGamer',   status: 'rejected', featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-15' },
  { id: 6,  title: 'Puzzle Master',     developer: 'CryptoGamer42',     status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-19' },
  { id: 7,  title: 'Racing Pro',        developer: 'NeonRacer99',       status: 'active',   featured: false, players: 5621,  revenue: 8430.00,  submittedAt: '2024-03-01' },
  { id: 8,  title: 'Chess Champions',   developer: 'ProStreamer',        status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-17' },
  { id: 9,  title: 'Fantasy RPG',       developer: 'StarForge Studios', status: 'active',   featured: true,  players: 12450, revenue: 24890.00, submittedAt: '2024-01-05' },
  { id: 10, title: 'Tower Defense',     developer: 'IndieDev_Mike',     status: 'rejected', featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-10' },
  { id: 11, title: 'Card Battle',       developer: 'CryptoGamer42',     status: 'pending',  featured: false, players: 0,     revenue: 0,        submittedAt: '2024-05-19' },
  { id: 12, title: 'Multiplayer Arena', developer: 'PixelMaster',       status: 'active',   featured: false, players: 3892,  revenue: 5840.00,  submittedAt: '2024-04-01' },
];

const STATUS_OPTIONS = [
  { value: 'all',      label: 'All Status' },
  { value: 'pending',  label: 'Pending'    },
  { value: 'active',   label: 'Active'     },
  { value: 'rejected', label: 'Rejected'   },
];

export default function Games() {
  const [statusFilter, setStatusFilter]     = useState('all');
  const [developerFilter, setDeveloperFilter] = useState('');
  const [currentPage, setCurrentPage]       = useState(1);
  const [actionLoading, setActionLoading]   = useState(null);
  const perPage = 10;

  const developers = useMemo(
    () => [...new Set(MOCK_GAMES.map(g => g.developer))].sort(),
    []
  );

  const filteredGames = useMemo(() => {
    let games = [...MOCK_GAMES];
    if (statusFilter !== 'all')  games = games.filter(g => g.status    === statusFilter);
    if (developerFilter)         games = games.filter(g => g.developer === developerFilter);
    return games;
  }, [statusFilter, developerFilter]);

  const paginatedGames = useMemo(() => {
    const start = (currentPage - 1) * perPage;
    return filteredGames.slice(start, start + perPage);
  }, [filteredGames, currentPage, perPage]);

  const pendingCount = MOCK_GAMES.filter(g => g.status === 'pending').length;

  const handleAction = async (_action, gameId) => {
    setActionLoading(gameId);
    await new Promise(r => setTimeout(r, 500));
    setActionLoading(null);
  };

  const statusBadgeClass = (status) => ({
    active:   'status-badge active',
    pending:  'status-badge pending',
    rejected: 'status-badge rejected',
  }[status] || 'status-badge');

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main">
          <header className="admin-header">
            <div>
              <span className="kicker">// PLATFORM CONTROL</span>
              <h1>Game Management</h1>
              <p>Review and manage Rust games</p>
            </div>
            {pendingCount > 0 && (
              <div className="pending-badge" role="status">
                {pendingCount} game{pendingCount !== 1 ? 's' : ''} pending review
              </div>
            )}
          </header>

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
                        {game.status === 'pending' && (
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
                        {game.status === 'active' && (
                          <Button
                            variant={game.featured ? 'secondary' : 'primary'}
                            size="sm"
                            loading={actionLoading === game.id}
                            onClick={() => handleAction('feature', game.id)}
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
