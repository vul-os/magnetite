import { useState, useMemo } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
import './admin.css';

const MOCK_USERS = [
  { id: 1,  username: 'CryptoGamer42',  email: 'crypto@example.com',      verified: true,  developer: true,  banned: false, createdAt: '2024-01-15', games: 3  },
  { id: 2,  username: 'PixelMaster',    email: 'pixel@example.com',        verified: true,  developer: true,  banned: false, createdAt: '2024-02-20', games: 7  },
  { id: 3,  username: 'NeonRacer99',    email: 'neon@example.com',         verified: true,  developer: false, banned: false, createdAt: '2024-03-10', games: 0  },
  { id: 4,  username: 'Cheater123',     email: 'cheater@example.com',      verified: false, developer: false, banned: true,  createdAt: '2024-03-15', games: 0  },
  { id: 5,  username: 'StarForge_Admin',email: 'admin@starforge.com',      verified: true,  developer: true,  banned: false, createdAt: '2024-01-01', games: 12 },
  { id: 6,  username: 'IndieDev_Mike',  email: 'mike@example.com',         verified: true,  developer: true,  banned: false, createdAt: '2024-04-01', games: 2  },
  { id: 7,  username: 'NewPlayer2024',  email: 'newbie@example.com',       verified: false, developer: false, banned: false, createdAt: '2024-05-10', games: 0  },
  { id: 8,  username: 'GameLover99',    email: 'lover@example.com',        verified: true,  developer: false, banned: false, createdAt: '2024-04-22', games: 0  },
  { id: 9,  username: 'BlockchainGamer',email: 'blockchain@example.com',   verified: true,  developer: true,  banned: false, createdAt: '2024-02-28', games: 4  },
  { id: 10, username: 'CasualPlayer',   email: 'casual@example.com',       verified: false, developer: false, banned: false, createdAt: '2024-05-15', games: 0  },
  { id: 11, username: 'ProStreamer',    email: 'stream@example.com',        verified: true,  developer: false, banned: false, createdAt: '2024-03-05', games: 0  },
  { id: 12, username: 'SuspiciousUser', email: 'suspicious@example.com',   verified: false, developer: false, banned: false, createdAt: '2024-05-18', games: 0  },
];

const FILTER_OPTIONS = [
  { value: 'all',        label: 'All Users'   },
  { value: 'verified',   label: 'Verified'    },
  { value: 'banned',     label: 'Banned'      },
  { value: 'developers', label: 'Developers'  },
];

const SORT_OPTIONS = [
  { value: 'date',     label: 'Date Joined' },
  { value: 'username', label: 'Username'    },
];

export default function Users() {
  const [search, setSearch]           = useState('');
  const [filter, setFilter]           = useState('all');
  const [sort, setSort]               = useState('date');
  const [currentPage, setCurrentPage] = useState(1);
  const [perPage, setPerPage]         = useState(10);
  const [actionLoading, setActionLoading] = useState(null);

  const filteredUsers = useMemo(() => {
    let users = [...MOCK_USERS];

    if (search) {
      const q = search.toLowerCase();
      users = users.filter(u =>
        u.username.toLowerCase().includes(q) || u.email.toLowerCase().includes(q)
      );
    }

    if (filter === 'verified')   users = users.filter(u => u.verified && !u.banned);
    if (filter === 'banned')     users = users.filter(u => u.banned);
    if (filter === 'developers') users = users.filter(u => u.developer);

    users.sort((a, b) =>
      sort === 'username'
        ? a.username.localeCompare(b.username)
        : new Date(b.createdAt) - new Date(a.createdAt)
    );

    return users;
  }, [search, filter, sort]);

  const paginatedUsers = useMemo(() => {
    const start = (currentPage - 1) * perPage;
    return filteredUsers.slice(start, start + perPage);
  }, [filteredUsers, currentPage, perPage]);

  const handleAction = async (_action, userId) => {
    setActionLoading(userId);
    await new Promise(r => setTimeout(r, 500));
    setActionLoading(null);
  };

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main">
          <header className="admin-header">
            <div>
              <span className="kicker">// PLATFORM CONTROL</span>
              <h1>User Management</h1>
              <p>Manage users and permissions</p>
            </div>
          </header>

          <div className="admin-toolbar">
            <div className="search-box">
              <input
                type="text"
                placeholder="Search by username or email..."
                value={search}
                onChange={(e) => { setSearch(e.target.value); setCurrentPage(1); }}
                aria-label="Search users"
              />
            </div>
            <div className="filter-controls">
              <select
                value={filter}
                onChange={(e) => { setFilter(e.target.value); setCurrentPage(1); }}
                aria-label="Filter users"
              >
                {FILTER_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
              <select
                value={sort}
                onChange={(e) => setSort(e.target.value)}
                aria-label="Sort users"
              >
                {SORT_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>Sort: {opt.label}</option>
                ))}
              </select>
            </div>
          </div>

          <div className="admin-table-container">
            <table className="admin-table" aria-label="Users">
              <thead>
                <tr>
                  <th>User</th>
                  <th>Status</th>
                  <th>Role</th>
                  <th>Games</th>
                  <th>Joined</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {paginatedUsers.map(user => (
                  <tr key={user.id}>
                    <td>
                      <div className="user-cell">
                        <span className="user-avatar" aria-hidden="true">
                          {user.username.charAt(0).toUpperCase()}
                        </span>
                        <div className="user-info">
                          <span className="user-name">{user.username}</span>
                          <span className="user-email">{user.email}</span>
                        </div>
                      </div>
                    </td>
                    <td>
                      {user.banned ? (
                        <span className="status-badge banned">Banned</span>
                      ) : user.verified ? (
                        <span className="status-badge verified">Verified</span>
                      ) : (
                        <span className="status-badge pending">Pending</span>
                      )}
                    </td>
                    <td>
                      {user.developer ? (
                        <span className="role-badge developer">Developer</span>
                      ) : (
                        <span className="role-badge player">Player</span>
                      )}
                    </td>
                    <td>
                      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)' }}>
                        {user.games}
                      </span>
                    </td>
                    <td>
                      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                        {user.createdAt}
                      </span>
                    </td>
                    <td>
                      <div className="action-buttons">
                        <Button variant="ghost" size="sm">View</Button>
                        {!user.banned ? (
                          <Button
                            variant="danger"
                            size="sm"
                            loading={actionLoading === user.id}
                            onClick={() => handleAction('ban', user.id)}
                          >
                            Ban
                          </Button>
                        ) : (
                          <Button
                            variant="secondary"
                            size="sm"
                            loading={actionLoading === user.id}
                            onClick={() => handleAction('unban', user.id)}
                          >
                            Unban
                          </Button>
                        )}
                        {!user.verified && !user.banned && (
                          <Button
                            variant="primary"
                            size="sm"
                            loading={actionLoading === user.id}
                            onClick={() => handleAction('verify', user.id)}
                          >
                            Verify
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
            total={filteredUsers.length}
            perPage={perPage}
            currentPage={currentPage}
            onPageChange={setCurrentPage}
            showPerPageSelector
            perPageOptions={[10, 25, 50]}
            onPerPageChange={(val) => { setPerPage(val); setCurrentPage(1); }}
          />
        </main>
      </div>
    </Layout>
  );
}
