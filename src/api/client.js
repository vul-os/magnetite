const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

async function request(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  const headers = {
    'Content-Type': 'application/json',
    ...(token && { Authorization: `Bearer ${token}` }),
    ...options.headers,
  };

  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers,
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ message: 'Request failed' }));
    throw new Error(error.message);
  }

  return response.json();
}

export const getOAuthUrl = (provider) => {
  return `${API_BASE}/api/auth/${provider}`;
};

export const api = {
  auth: {
    register: (data) => request('/api/auth/register', { method: 'POST', body: JSON.stringify(data) }),
    login: (data) => request('/api/auth/login', { method: 'POST', body: JSON.stringify(data) }),
    me: () => request('/api/auth/me'),
    linkedAccounts: () => request('/api/auth/linked-accounts'),
    linkAccount: (token) => request('/api/auth/linked-accounts', { method: 'POST', body: JSON.stringify({ token }) }),
    unlinkAccount: (id) => request(`/api/auth/linked-accounts/${id}`, { method: 'DELETE' }),
    forgotPassword: (email) => request('/api/auth/forgot-password', { method: 'POST', body: JSON.stringify({ email }) }),
    resetPassword: (token, password) => request('/api/auth/reset-password', { method: 'POST', body: JSON.stringify({ token, password }) }),
    verifyEmail: (token) => request('/api/auth/verify-email', { method: 'POST', body: JSON.stringify({ token }) }),
    resendVerification: (email) => request('/api/auth/resend-verification', { method: 'POST', body: JSON.stringify({ email }) }),
    updatePassword: (currentPassword, newPassword) => request('/api/auth/password', { method: 'PUT', body: JSON.stringify({ currentPassword, newPassword }) }),
  },
  wallet: {
    balance: () => request('/api/wallet/balance'),
    deposit: (data) => request('/api/wallet/deposit', { method: 'POST', body: JSON.stringify(data) }),
    withdraw: (data) => request('/api/wallet/withdraw', { method: 'POST', body: JSON.stringify(data) }),
    transactions: () => request('/api/wallet/transactions'),
  },
  games: {
    list: () => request('/api/games'),
    get: (id) => request(`/api/games/${id}`),
    create: (data) => request('/api/games', { method: 'POST', body: JSON.stringify(data) }),
    update: (id, data) => request(`/api/games/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id) => request(`/api/games/${id}`, { method: 'DELETE' }),
    leaderboard: (id) => request(`/api/games/${id}/leaderboard`),
  },
  matchmaking: {
    join: (gameId) => request('/api/matchmaking/join', { method: 'POST', body: JSON.stringify({ game_id: gameId }) }),
    leave: () => request('/api/matchmaking/leave', { method: 'DELETE' }),
    status: () => request('/api/matchmaking/status'),
  },
  subscriptions: {
    plans: () => request('/api/subscriptions/plans'),
    current: () => request('/api/subscriptions/current'),
    create: (data) => request('/api/subscriptions', { method: 'POST', body: JSON.stringify(data) }),
    cancel: () => request('/api/subscriptions/cancel', { method: 'POST' }),
    upgrade: (planId) => request('/api/subscriptions/upgrade', { method: 'POST', body: JSON.stringify({ plan_id: planId }) }),
    hours: () => request('/api/subscriptions/hours'),
    usage: () => request('/api/subscriptions/usage'),
  },
  search: {
    query: (q, searchType = 'all', limit = 20, offset = 0) =>
      request(`/api/search?q=${encodeURIComponent(q)}&search_type=${searchType}&limit=${limit}&offset=${offset}`),
  },
};