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
const MOCK_USERS = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 1,  username: 'CryptoGamer42',   email: 'crypto@example.com',    verified: true,  developer: true,  banned: false, createdAt: '2024-01-15', games: 3  },
      { id: 2,  username: 'PixelMaster',     email: 'pixel@example.com',     verified: true,  developer: true,  banned: false, createdAt: '2024-02-20', games: 7  },
      { id: 3,  username: 'NeonRacer99',     email: 'neon@example.com',      verified: true,  developer: false, banned: false, createdAt: '2024-03-10', games: 0  },
      { id: 4,  username: 'Cheater123',      email: 'cheater@example.com',   verified: false, developer: false, banned: true,  createdAt: '2024-03-15', games: 0  },
      { id: 5,  username: 'StarForge_Admin', email: 'admin@starforge.com',   verified: true,  developer: true,  banned: false, createdAt: '2024-01-01', games: 12 },
    ]
  : null;

const FILTER_OPTIONS = [
  { value: 'all',        label: 'All Users'  },
  { value: 'verified',   label: 'Verified'   },
  { value: 'banned',     label: 'Banned'     },
  { value: 'developers', label: 'Developers' },
];

const SORT_OPTIONS = [
  { value: 'date',     label: 'Date Joined' },
  { value: 'username', label: 'Username'    },
];

function normaliseUser(u) {
  return {
    id:        u.id,
    username:  u.username,
    email:     u.email,
    verified:  u.banned_at == null,
    developer: u.is_developer ?? false,
    banned:    u.banned_at != null,
    createdAt: u.created_at ? u.created_at.split('T')[0] : '',
    games:     u.games ?? 0,
  };
}

export default function Users() {
  const [users, setUsers]             = useState(MOCK_USERS ? MOCK_USERS.map(u => ({ ...u })) : []);
  const [loading, setLoading]         = useState(!MOCK_USERS);
  const [error, setError]             = useState(null);
  const [search, setSearch]           = useState('');
  const [filter, setFilter]           = useState('all');
  const [sort, setSort]               = useState('date');
  const [currentPage, setCurrentPage] = useState(1);
  const [perPage, setPerPage]         = useState(10);
  const [actionLoading, setActionLoading] = useState(null);
  const [actionError, setActionError]     = useState(null);

  const fetchUsers = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    setLoading(true);
    setError(null);
    try {
      const res = await authFetch('/api/admin/users?limit=200');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = await res.json();
      const raw = json.data ?? json ?? [];
      setUsers(Array.isArray(raw) ? raw.map(normaliseUser) : []);
    } catch (err) {
      setError(err.message || 'Failed to load users');
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch users from the admin API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchUsers(); }, [fetchUsers]);

  const filteredUsers = useMemo(() => {
    let list = [...users];
    if (search) {
      const q = search.toLowerCase();
      list = list.filter(u =>
        u.username.toLowerCase().includes(q) || u.email.toLowerCase().includes(q)
      );
    }
    if (filter === 'verified')   list = list.filter(u => u.verified && !u.banned);
    if (filter === 'banned')     list = list.filter(u => u.banned);
    if (filter === 'developers') list = list.filter(u => u.developer);
    list.sort((a, b) =>
      sort === 'username'
        ? a.username.localeCompare(b.username)
        : new Date(b.createdAt) - new Date(a.createdAt)
    );
    return list;
  }, [users, search, filter, sort]);

  const paginatedUsers = useMemo(() => {
    const start = (currentPage - 1) * perPage;
    return filteredUsers.slice(start, start + perPage);
  }, [filteredUsers, currentPage, perPage]);

  const handleAction = async (action, userId) => {
    setActionLoading(userId);
    setActionError(null);
    try {
      let res;
      if (action === 'ban') {
        res = await authFetch(`/api/admin/users/${userId}/ban`, {
          method: 'PUT',
          body: JSON.stringify({ banned: true }),
        });
      } else if (action === 'unban') {
        res = await authFetch(`/api/admin/users/${userId}/ban`, {
          method: 'PUT',
          body: JSON.stringify({ banned: false }),
        });
      } else if (action === 'verify') {
        res = await authFetch(`/api/admin/users/${userId}/role`, {
          method: 'PUT',
          body: JSON.stringify({ role: 'user' }),
        });
      }
      if (res && !res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || `Action failed (HTTP ${res.status})`);
      }
      /* optimistic update */
      setUsers(prev => prev.map(u => {
        if (u.id !== userId && String(u.id) !== String(userId)) return u;
        if (action === 'ban')    return { ...u, banned: true,  verified: false };
        if (action === 'unban')  return { ...u, banned: false };
        if (action === 'verify') return { ...u, verified: true };
        return u;
      }));
    } catch (err) {
      setActionError(err.message);
    } finally {
      setActionLoading(null);
    }
  };

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Platform Control</span>
              <h1>User Management</h1>
              <p>Manage users and permissions</p>
            </div>
          </header>

          {error && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {error}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={fetchUsers}>
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
            {loading ? (
              <div className="admin-loading" aria-busy="true" style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>
                <span className="spinner" aria-hidden="true" /> Loading users&hellip;
              </div>
            ) : filteredUsers.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                No users found
              </div>
            ) : (
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
            )}
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
