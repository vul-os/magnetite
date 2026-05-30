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
  notifications: {
    list: () => request('/api/notifications'),
    unreadCount: () => request('/api/notifications/count'),
    markAsRead: (id) => request(`/api/notifications/${id}/read`, { method: 'PUT' }),
    markAllAsRead: () => request('/api/notifications/read-all', { method: 'PUT' }),
    delete: (id) => request(`/api/notifications/${id}`, { method: 'DELETE' }),
  },
  achievements: {
    list: (userId) => request(`/api/achievements/${userId}`),
    get: (userId, id) => request(`/api/achievements/${userId}/${id}`),
    leaderboard: () => request('/api/achievements/leaderboard'),
  },
  profile: {
    get: (username) => request(`/api/users/${username}`),
    update: (data) => request('/api/auth/profile', { method: 'PUT', body: JSON.stringify(data) }),
  },
  social: {
    friends: () => request('/api/friends'),
    addFriend: (userId) => request('/api/friends', { method: 'POST', body: JSON.stringify({ user_id: userId }) }),
    removeFriend: (userId) => request(`/api/friends/${userId}`, { method: 'DELETE' }),
    searchUsers: (q) => request(`/api/users/search?q=${encodeURIComponent(q)}`),
    invites: () => request('/api/invites'),
    sendInvite: (userId, gameId) => request('/api/invites', { method: 'POST', body: JSON.stringify({ user_id: userId, game_id: gameId }) }),
    acceptInvite: (id) => request(`/api/invites/${id}/accept`, { method: 'POST' }),
    declineInvite: (id) => request(`/api/invites/${id}/decline`, { method: 'POST' }),
  },
  wishlist: {
    list: () => request('/api/wishlist'),
    add: (gameId) => request('/api/wishlist', { method: 'POST', body: JSON.stringify({ game_id: gameId }) }),
    remove: (gameId) => request(`/api/wishlist/${gameId}`, { method: 'DELETE' }),
  },
  reviews: {
    list: (gameId) => request(`/api/games/${gameId}/reviews`),
    create: (gameId, data) => request(`/api/games/${gameId}/reviews`, { method: 'POST', body: JSON.stringify(data) }),
  },
  developer: {
    dashboard: () => request('/api/developer/dashboard'),
    games: () => request('/api/developer/games'),
    earnings: () => request('/api/developer/earnings'),
    analytics: (gameId) => request(`/api/developer/analytics/${gameId}`),
  },

  // ── Wave 6: Comms Core ────────────────────────────────────────────────────

  communities: {
    /** List all communities the current user is a member of. */
    list: () => request('/api/communities'),
    /** Fetch a single community by id. */
    get: (id) => request(`/api/communities/${id}`),
    /** Create a new community. data: { name, description?, icon_url? } */
    create: (data) => request('/api/communities', { method: 'POST', body: JSON.stringify(data) }),
    /** Join a community by invite code or id. */
    join: (id) => request(`/api/communities/${id}/join`, { method: 'POST' }),
    /** Leave a community. */
    leave: (id) => request(`/api/communities/${id}/leave`, { method: 'DELETE' }),
    /** List members of a community. */
    members: (id) => request(`/api/communities/${id}/members`),
  },

  channels: {
    /** List channels within a community. */
    list: (communityId) => request(`/api/communities/${communityId}/channels`),
    /** Create a channel inside a community. data: { name, kind } where kind = 'text' | 'voice' */
    create: (communityId, data) =>
      request(`/api/communities/${communityId}/channels`, { method: 'POST', body: JSON.stringify(data) }),
  },

  messages: {
    /** List messages in a channel (paginated). params: { limit?, before? } */
    list: (channelId, params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/channels/${channelId}/messages${qs ? `?${qs}` : ''}`);
    },
    /** Post a message to a channel. data: { content } */
    post: (channelId, data) =>
      request(`/api/channels/${channelId}/messages`, { method: 'POST', body: JSON.stringify(data) }),
    /** List DM messages between the current user and another. */
    listDMs: (userId, params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/dms/${userId}${qs ? `?${qs}` : ''}`);
    },
    /** Send a DM to another user. data: { content } */
    sendDM: (userId, data) =>
      request(`/api/dms/${userId}`, { method: 'POST', body: JSON.stringify(data) }),
  },

  voice: {
    /** List voice rooms in a community. */
    rooms: (communityId) => request(`/api/communities/${communityId}/voice-rooms`),
    /** Obtain a join token for a voice room. Returns { token, room_id }. */
    joinToken: (roomId) => request(`/api/voice-rooms/${roomId}/join`, { method: 'POST' }),
  },

  streams: {
    /** List active streams in a community. */
    list: (communityId) => request(`/api/communities/${communityId}/streams`),
    /** Start streaming in a channel. data: { channel_id, title? } */
    goLive: (communityId, data) =>
      request(`/api/communities/${communityId}/streams`, { method: 'POST', body: JSON.stringify(data) }),
  },

  // ── Wave 8: Points / Score Economy ───────────────────────────────────────

  points: {
    /** Current user's point balance and season info. */
    balance: () => request('/api/points/balance'),
    /** Paginated points history. params: { limit?, offset? } */
    history: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/points/history${qs ? `?${qs}` : ''}`);
    },
    /** Award points (admin / game-server). data: { user_id, amount, reason } */
    award: (data) => request('/api/points/award', { method: 'POST', body: JSON.stringify(data) }),
    /** Global points leaderboard. params: { limit?, game_id? } */
    leaderboard: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/points/leaderboard${qs ? `?${qs}` : ''}`);
    },
    /** Redeemable rewards catalogue. */
    rewards: () => request('/api/points/rewards'),
    /** Redeem a reward. data: { reward_id } */
    redeem: (data) => request('/api/points/redeem', { method: 'POST', body: JSON.stringify(data) }),
  },

  // ── Wave 8: Marketplace Stores & Items ───────────────────────────────────

  stores: {
    /** List all public stores. params: { game_id?, limit?, offset? } */
    list: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/stores${qs ? `?${qs}` : ''}`);
    },
    /** Fetch a single store. */
    get: (storeId) => request(`/api/stores/${storeId}`),
    /** Create a developer store. data: { name, game_id, description? } */
    create: (data) => request('/api/stores', { method: 'POST', body: JSON.stringify(data) }),
    /** Update store metadata. */
    update: (storeId, data) => request(`/api/stores/${storeId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** Delete a store. */
    delete: (storeId) => request(`/api/stores/${storeId}`, { method: 'DELETE' }),
    /** List items in a store. */
    items: (storeId) => request(`/api/stores/${storeId}/items`),
    /** Add an item to a store. data: { name, description, price_points, price_usdc, item_type, metadata? } */
    addItem: (storeId, data) =>
      request(`/api/stores/${storeId}/items`, { method: 'POST', body: JSON.stringify(data) }),
    /** Update a store item. */
    updateItem: (storeId, itemId, data) =>
      request(`/api/stores/${storeId}/items/${itemId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** Remove an item from a store. */
    removeItem: (storeId, itemId) =>
      request(`/api/stores/${storeId}/items/${itemId}`, { method: 'DELETE' }),
    /** Purchase an item. data: { currency: 'points' | 'usdc' } */
    purchase: (storeId, itemId, data) =>
      request(`/api/stores/${storeId}/items/${itemId}/purchase`, { method: 'POST', body: JSON.stringify(data) }),
    /** Current user's entitlements (purchased items). */
    entitlements: () => request('/api/stores/entitlements'),
    /** Developer sales summary for owned stores. */
    sales: (storeId) => request(`/api/stores/${storeId}/sales`),
  },
};
