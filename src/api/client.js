const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

/**
 * Normalise an endpoint path so that any /api/... path that is NOT already
 * /api/v1/... gets rewritten to /api/v1/...  This fixes the 64-call prefix
 * mismatch in a single place without changing callers.
 */
function normaliseEndpoint(endpoint) {
  if (endpoint.startsWith('/api/') && !endpoint.startsWith('/api/v1/')) {
    return '/api/v1/' + endpoint.slice('/api/'.length);
  }
  return endpoint;
}

async function request(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  const headers = {
    'Content-Type': 'application/json',
    ...(token && { Authorization: `Bearer ${token}` }),
    ...options.headers,
  };

  const normalisedEndpoint = normaliseEndpoint(endpoint);

  const response = await fetch(`${API_BASE}${normalisedEndpoint}`, {
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
  return `${API_BASE}/api/v1/oauth/${provider}`;
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
    /** List API keys for the authenticated user. GET /api/auth/api-keys */
    apiKeys: () => request('/api/auth/api-keys'),
    /** Create a new API key. POST /api/auth/api-keys — returns { id, name, key } (one-time) */
    createApiKey: (name) => request('/api/auth/api-keys', { method: 'POST', body: JSON.stringify({ name }) }),
    /** Revoke an API key by id. DELETE /api/auth/api-keys/:id */
    revokeApiKey: (id) => request(`/api/auth/api-keys/${id}`, { method: 'DELETE' }),
    /** Begin 2FA TOTP setup. POST /api/auth/2fa/setup — returns { otpauth_uri, qr_data_url? } */
    setup2fa: () => request('/api/auth/2fa/setup', { method: 'POST' }),
    /** Verify and enable 2FA. POST /api/auth/2fa/verify */
    verify2fa: (code) => request('/api/auth/2fa/verify', { method: 'POST', body: JSON.stringify({ code }) }),
    /** Disable 2FA. POST /api/v1/auth/2fa/disable */
    disable2fa: (code) => request('/api/v1/auth/2fa/disable', { method: 'POST', body: JSON.stringify({ code }) }),
  },
  /**
   * Wallet — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
   *
   * This node holds no funds. A "wallet" is nothing but the Ed25519 address the
   * user has linked so a checkout can pay them (or charge them) directly,
   * wallet→wallet. There is no balance, no deposit, no withdrawal, no payout:
   * `/wallet/balance`, `/wallet/deposit`, `/wallet/transactions` and
   * `/wallet/withdraw` were all removed from the backend.
   */
  wallet: {
    /** GET /api/v1/wallet → { user_id, wallet_address, custodial: false, rail } */
    get: () => request('/api/v1/wallet'),
    /** POST /api/v1/wallet/link — link/replace the hex Ed25519 address. */
    link: (walletAddress) =>
      request('/api/v1/wallet/link', {
        method: 'POST',
        body: JSON.stringify({ wallet_address: walletAddress }),
      }),
    /**
     * GET /api/v1/wallet/receipts — the signed receipts this user paid for.
     * Replaces the custodial transaction ledger. Each row:
     * { id, kind, total, protocol_fee, rail_pubkey, voided, created_at }
     */
    receipts: () => request('/api/v1/wallet/receipts'),
    /** POST /api/v1/wallet/hosting/pay — pay an operator's hosting fee (§3.6b). */
    payHostingFee: ({ operatorPubkey, amount, serverId }) =>
      request('/api/v1/wallet/hosting/pay', {
        method: 'POST',
        body: JSON.stringify({
          operator_pubkey: operatorPubkey,
          amount,
          server_id: serverId,
        }),
      }),
    /** GET /api/v1/wallet/hosting/:serverId → { server_id, allowed } */
    hostingAccess: (serverId) => request(`/api/v1/wallet/hosting/${serverId}`),
  },

  /**
   * Discovery — the phonebook (seam §3.4). Nodes self-advertise `SessionAd`s;
   * this is a hint layer, never an authority. Replaces the old central
   * `runtime_instances` poll.
   */
  discovery: {
    /**
     * GET /api/v1/discovery/sessions — discovered session ads. Each ad:
     * { game, node, capacity: { cpu_cores, ram_mb, bandwidth_mbps, free_slots,
     *   max_shards }, ping_hint, price, chat_room, voice_room }
     */
    sessions: (filter = {}) => {
      const qs = new URLSearchParams(
        Object.entries(filter).filter(([, v]) => v != null && v !== ''),
      ).toString();
      return request(`/api/v1/discovery/sessions${qs ? `?${qs}` : ''}`);
    },
    /** POST /api/v1/discovery/announce — a node advertises a session it hosts. */
    announce: (ad) =>
      request('/api/v1/discovery/announce', { method: 'POST', body: JSON.stringify(ad) }),
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
    /** GET /api/v1/subscriptions — list available tiers/plans */
    plans: () => request('/api/v1/subscriptions'),
    /** GET /api/v1/subscriptions/me — current user's active subscription */
    current: () => request('/api/v1/subscriptions/me'),
    create: (data) => request('/api/subscriptions', { method: 'POST', body: JSON.stringify(data) }),
    /**
     * DELETE /api/v1/subscriptions — drop back to the free tier.
     * Note: nothing is "cancelled" in a billing sense — nobody holds a mandate
     * against the user's wallet. A tier is a receipt-backed feature flag that
     * lapses on its own; this just relinquishes it early.
     */
    cancel: () => request('/api/v1/subscriptions', { method: 'DELETE' }),
    /**
     * POST /api/v1/subscriptions/upgrade — switch tier.
     * planId: target tier id.
     * receiptId: id of the signed `payment_receipts` row that paid the operator
     *   wallet for the new tier (omitted when moving to a cheaper/free tier).
     * There is no proration charge: nothing is billed in arrears.
     */
    upgrade: (planId, receiptId) =>
      request('/api/v1/subscriptions/upgrade', {
        method: 'POST',
        body: JSON.stringify({
          plan_id: planId,
          ...(receiptId ? { receipt_id: receiptId } : {}),
        }),
      }),
    /**
     * /hours and /usage have no backend implementation (AUDIT high).
     * Kept here so the UI can display an honest error; will be implemented in a later wave.
     */
    hours: () => request('/api/v1/subscriptions/hours'),
    usage: () => request('/api/v1/subscriptions/usage'),
  },
  search: {
    /**
     * GET /api/v1/search — full-text game/user search.
     * q: search query string.
     * searchType: 'all' | 'game' | 'user'.
     * filters: { genre?, tags?, min_rating?, is_free? } — genre/tag filter (AX2).
     * limit/offset: pagination.
     */
    query: (q, searchType = 'all', limit = 20, offset = 0, filters = {}) => {
      const params = new URLSearchParams({
        q,
        search_type: searchType,
        limit: String(limit),
        offset: String(offset),
      });
      if (filters.genre) params.set('genre', filters.genre);
      if (filters.tags) params.set('tags', filters.tags);
      if (filters.min_rating != null) params.set('min_rating', String(filters.min_rating));
      if (filters.is_free != null) params.set('is_free', String(filters.is_free));
      return request(`/api/search?${params.toString()}`);
    },
  },
  notifications: {
    list: () => request('/api/notifications'),
    unreadCount: () => request('/api/notifications/count'),
    markAsRead: (id) => request(`/api/notifications/${id}/read`, { method: 'PUT' }),
    markAllAsRead: () => request('/api/notifications/read-all', { method: 'PUT' }),
    delete: (id) => request(`/api/notifications/${id}`, { method: 'DELETE' }),
    /**
     * GET /api/v1/notifications/preferences
     * Returns the authenticated user's per-channel, per-category notification
     * preferences.  A default row is created on first access.
     */
    getPreferences: () => request('/api/v1/notifications/preferences'),
    /**
     * PUT /api/v1/notifications/preferences
     * Partial update — only supply the fields you want to change.
     * data: Partial<{
     *   payouts_email, payouts_in_app, payouts_push,
     *   social_email, social_in_app, social_push,
     *   achievements_email, achievements_in_app, achievements_push,
     *   marketing_email, marketing_in_app, marketing_push,
     * }>
     */
    updatePreferences: (data) =>
      request('/api/v1/notifications/preferences', { method: 'PUT', body: JSON.stringify(data) }),
  },
  achievements: {
    list: (userId) => request(`/api/achievements/${userId}`),
    get: (userId, id) => request(`/api/achievements/${userId}/${id}`),
    leaderboard: () => request('/api/achievements/leaderboard'),
  },
  profile: {
    /** GET /api/v1/users/by-username/:username — look up profile by username string */
    get: (username) => request(`/api/v1/users/by-username/${encodeURIComponent(username)}`),
    /** PUT /api/v1/profile — update the authenticated user's profile */
    update: (data) => request('/api/v1/profile', { method: 'PUT', body: JSON.stringify(data) }),
  },
  social: {
    friends: () => request('/api/friends'),
    /** GET /api/v1/friends/pending — incoming friend requests (to_user_id = me, status = pending) */
    pendingRequests: () => request('/api/v1/friends/pending'),
    /** GET /api/v1/friends/sent — outgoing friend requests (from_user_id = me, status = pending) */
    sentRequests: () => request('/api/v1/friends/sent'),
    /** DELETE /api/v1/friends/request/:id — cancel a sent friend request */
    cancelRequest: (id) => request(`/api/v1/friends/request/${id}`, { method: 'DELETE' }),
    /** POST /api/v1/friends/accept/:id — accept a pending incoming request */
    acceptRequest: (id) => request(`/api/v1/friends/accept/${id}`, { method: 'POST' }),
    /** POST /api/v1/friends/reject/:id — decline a pending incoming request */
    rejectRequest: (id) => request(`/api/v1/friends/reject/${id}`, { method: 'POST' }),
    /** POST /api/v1/friends/request — send a friend request; body uses to_user_id per backend */
    addFriend: (userId) => request('/api/v1/friends/request', { method: 'POST', body: JSON.stringify({ to_user_id: userId }) }),
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
    /** Mark a review as helpful. POST /api/games/:gameId/reviews/:reviewId/helpful */
    helpful: (gameId, reviewId) =>
      request(`/api/games/${gameId}/reviews/${reviewId}/helpful`, { method: 'POST' }),
    /** Report a review. POST /api/games/:gameId/reviews/:reviewId/report */
    report: (gameId, reviewId) =>
      request(`/api/games/${gameId}/reviews/${reviewId}/report`, { method: 'POST' }),
  },
  contact: {
    /** Submit a contact form message. POST /api/v1/contact */
    submit: (data) => request('/api/v1/contact', { method: 'POST', body: JSON.stringify(data) }),
  },

  platform: {
    /** Get platform settings (admin). GET /api/platform/settings */
    getSettings: () => request('/api/platform/settings'),
    /** Update platform settings (admin). PUT /api/platform/settings */
    updateSettings: (data) => request('/api/platform/settings', { method: 'PUT', body: JSON.stringify(data) }),
  },

  admin: {
    /**
     * GET /api/v1/admin/review-reports — list reported reviews (admin only).
     * params: { limit?, offset?, status?, reason? }
     */
    reviewReports: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/admin/review-reports${qs ? `?${qs}` : ''}`);
    },
    /**
     * POST /api/v1/admin/review-reports/:id/dismiss — dismiss a report or remove the review.
     * data: { action: 'dismiss' | 'remove_review' | 'ban_user' }
     */
    dismissReport: (reportId, data) =>
      request(`/api/v1/admin/review-reports/${reportId}/dismiss`, { method: 'POST', body: JSON.stringify(data) }),
    /**
     * GET /api/v1/admin/users — list users (admin only).
     * params: { limit?, offset?, filter?, sort? }
     */
    users: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/admin/users${qs ? `?${qs}` : ''}`);
    },
    /** POST /api/v1/admin/users/:id/ban — ban a user */
    banUser: (userId, reason) =>
      request(`/api/v1/admin/users/${userId}/ban`, { method: 'POST', body: JSON.stringify({ reason }) }),
    /** POST /api/v1/admin/users/:id/unban — unban a user */
    unbanUser: (userId) =>
      request(`/api/v1/admin/users/${userId}/unban`, { method: 'POST' }),
    /**
     * POST /api/v1/admin/transactions/:id/refund — initiate a refund for a transaction.
     * data: { reason?: string }
     * Returns a refund_records row: { id, transaction_id, user_id, amount, provider, status, ... }
     */
    refundTransaction: (transactionId, data = {}) =>
      request(`/api/v1/admin/transactions/${transactionId}/refund`, { method: 'POST', body: JSON.stringify(data) }),

    /**
     * POST /api/v1/admin/users/:id/warn — issue a warning to a user.
     * data: { reason: string }
     */
    warnUser: (userId, reason) =>
      request(`/api/v1/admin/users/${userId}/warn`, { method: 'POST', body: JSON.stringify({ reason }) }),

    /**
     * GET /api/v1/admin/review-reports/:id — get a single report detail.
     */
    getReport: (reportId) =>
      request(`/api/v1/admin/review-reports/${reportId}`),
  },

  developer: {
    dashboard: () => request('/api/developer/dashboard'),
    games: () => request('/api/developer/games'),
    earnings: () => request('/api/developer/earnings'),
    payouts: () => request('/api/developer/payouts'),
    analytics: (gameId) => request(`/api/developer/analytics/${gameId}`),

    /**
     * GET /api/v1/developer/games/:id/analytics
     * Returns per-game 30-day analytics:
     *   { game_id, game_title, summary: { total_revenue, active_players, total_sessions },
     *     daily_revenue: [{ date, revenue }],
     *     daily_playtime: [{ date, minutes }] }
     * params: { from?, to? } — ISO date strings for date-range filtering
     */
    gameAnalytics: (gameId, params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/developer/games/${gameId}/analytics${qs ? `?${qs}` : ''}`);
    },

    // ── GDS: Game scaffold ───────────────────────────────────────────────────
    /**
     * POST /api/v1/developer/games/scaffold
     * body: { name: string, template_id: string, description?: string }
     * Returns: { game_id, name, template_id, created_at, cli_instructions: string, next_steps: string[] }
     */
    scaffold: (data) =>
      request('/api/v1/developer/games/scaffold', { method: 'POST', body: JSON.stringify(data) }),

    /**
     * GET /api/v1/developer/games/:gameId/builds
     * Returns list of build jobs: { id, status, version, commit_sha, started_at, logs_url }
     */
    builds: (gameId) => request(`/api/v1/developer/games/${gameId}/builds`),

    /**
     * GET /api/v1/developer/games/:gameId/builds/:buildId/logs
     * Returns { logs: string, status, updated_at }
     */
    buildLogs: (gameId, buildId) => request(`/api/v1/developer/games/${gameId}/builds/${buildId}/logs`),

    /**
     * POST /api/v1/developer/games/:gameId/builds/:buildId/promote
     * Promotes this build to be the live version.
     */
    promote: (gameId, buildId) =>
      request(`/api/v1/developer/games/${gameId}/builds/${buildId}/promote`, { method: 'POST' }),

    /**
     * POST /api/v1/developer/games/:gameId/rollback
     * body: { version: string }
     */
    rollback: (gameId, data) =>
      request(`/api/v1/developer/games/${gameId}/rollback`, { method: 'POST', body: JSON.stringify(data) }),

    // Payout recipients are GONE. Developers are paid wallet-to-wallet at
    // checkout (seam §3.6), so there is no bank account to register, no
    // recipient to maintain and no payout to schedule. The developer's
    // destination is simply the wallet address linked via `api.wallet.link`.

    // ── Payout request (D-PAY-4) ─────────────────────────────────────────
    /**
     * POST /api/v1/developer/payouts
     * data: { amount: number }
     * Creates a payout_requests row; processed async by the payout job via Wise.
     */
    requestPayout: (data) =>
      request('/api/v1/developer/payouts', { method: 'POST', body: JSON.stringify(data) }),
    /**
     * GET /api/v1/developer/payouts — list payout requests with status.
     * (Was /payout-status — that route does not exist; /payouts serves both create and history.)
     */
    payoutStatus: () => request('/api/v1/developer/payouts'),
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
    /**
     * GET /api/v1/communities/:id/voice-rooms — list voice channels in a community.
     * NOTE: this REST endpoint is being added by agent 2 (AX1 backend wave).
     * Until it is live the call will return an honest 404.
     */
    rooms: (communityId) => request(`/api/v1/communities/${communityId}/voice-rooms`),
    /**
     * POST /api/v1/voice-rooms/:id/join — obtain a join token for a voice room.
     * NOTE: this REST endpoint is being added by agent 2.
     */
    joinToken: (roomId) => request(`/api/v1/voice-rooms/${roomId}/join`, { method: 'POST' }),
  },

  streams: {
    /**
     * List live streams.
     * communityId = 'global' → platform-wide listing (/api/v1/streams/live)
     * communityId = specific id → community-scoped listing (/api/v1/communities/:id/streams)
     * NOTE: community-scoped stream routes are being added by agent 2.
     */
    list: (communityId) =>
      communityId === 'global'
        ? request('/api/v1/streams/live').catch(() => request('/api/v1/streams'))
        : request(`/api/v1/communities/${communityId}/streams`),

    /**
     * Start streaming.
     * Tries community-scoped endpoint first (being added by agent 2);
     * falls back to /api/v1/streams for global/un-scoped streams.
     */
    goLive: (communityId, data) =>
      communityId && communityId !== 'global'
        ? request(`/api/v1/communities/${communityId}/streams`, { method: 'POST', body: JSON.stringify(data) })
        : request('/api/v1/streams', { method: 'POST', body: JSON.stringify(data) }),

    /** Stop / end a stream by id. DELETE /api/v1/streams/:id */
    end: (streamId) => request(`/api/v1/streams/${streamId}`, { method: 'DELETE' }),

    /**
     * Get stream detail (title, status, etc). There is no /watch sub-route on the backend.
     * Use hlsUrl() to obtain the HLS playlist for playback.
     * GET /api/v1/streams/:id
     */
    watch: (streamId) => request(`/api/v1/streams/${streamId}`),

    /**
     * Get the canonical HLS playlist URL.
     * Backend registers /:id/hls — no /index.m3u8 suffix.
     */
    hlsUrl: (streamId) => {
      const base = import.meta.env.VITE_API_URL || 'http://localhost:8080';
      return `${base}/api/v1/streams/${streamId}/hls`;
    },
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

  distribution: {
    /**
     * GET /api/v1/distribution/:game_id/play
     * Returns { game_id, version, commit_sha, wasm_url, server_url, artifact_type, sha256_hash, file_size_bytes }.
     * `server_url` is the live WebSocket endpoint the browser should connect to.
     */
    playManifest: (gameId) => request(`/api/v1/distribution/${gameId}/play`),
  },

  // ── GDS: Game Templates + Scaffold ──────────────────────────────────────────

  templates: {
    /**
     * GET /api/v1/templates
     * Returns a list of game templates, each with:
     *   { id, name, description, tier, tags[], preview_url?, screenshot_url? }
     * tier: 'free' | 'starter' | 'advanced'
     */
    list: () => request('/api/v1/templates'),

    /**
     * GET /api/v1/templates/:id
     * Returns a single template detail.
     */
    get: (id) => request(`/api/v1/templates/${id}`),
  },

  // ── Wave REPLAY+TOURNAMENT: Replays ──────────────────────────────────────────

  replays: {
    /**
     * GET /api/v1/replays — list replay logs.
     * params: { game_id?, limit?, offset? }
     * Returns: { data: ReplaySummary[], total, page, per_page }
     *   ReplaySummary: { id, game_id, game_title, recorded_at, tick_count, duration_ms, verdict }
     */
    list: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/replays${qs ? `?${qs}` : ''}`);
    },

    /**
     * GET /api/v1/replays/:id — fetch a full ReplayLog.
     * Returns: { id, config: MatchConfig, frames: [Tick, [PlayerId, Input][]][], state_hashes: [Tick, u64][], recorded_at, verdict }
     */
    get: (id) => request(`/api/v1/replays/${id}`),

    /**
     * DELETE /api/v1/replays/:id — delete a replay (admin/owner only).
     */
    delete: (id) => request(`/api/v1/replays/${id}`, { method: 'DELETE' }),
  },

  // ── Wave REPLAY+TOURNAMENT: Tournaments ───────────────────────────────────────

  tournaments: {
    /**
     * GET /api/v1/tournaments — list tournaments.
     * params: { status?, game_id?, page?, per_page? }
     * Returns PaginatedResponse<Tournament>
     *   Tournament: { id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at }
     */
    list: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/tournaments${qs ? `?${qs}` : ''}`);
    },

    /**
     * GET /api/v1/tournaments/:id — get tournament details.
     * Returns: { tournament: Tournament, participants: TournamentParticipant[], matches: TournamentMatch[] }
     *   TournamentParticipant: { id, tournament_id, user_id, registered_at, status, seed }
     *   TournamentMatch: { id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
     *                      player1_score, player2_score, status, scheduled_at, completed_at }
     */
    get: (id) => request(`/api/v1/tournaments/${id}`),

    /**
     * POST /api/v1/tournaments — create a tournament (auth required).
     * data: { name, game_id, max_players?, entry_fee?, prize_pool?, start_time }
     * Returns: Tournament
     */
    create: (data) =>
      request('/api/v1/tournaments', { method: 'POST', body: JSON.stringify(data) }),

    /**
     * PUT /api/v1/tournaments/:id — update a tournament (Draft/Registration only; auth required).
     * data: { name?, status?, max_players?, entry_fee?, prize_pool?, start_time? }
     * Returns: Tournament
     */
    update: (id, data) =>
      request(`/api/v1/tournaments/${id}`, { method: 'PUT', body: JSON.stringify(data) }),

    /**
     * POST /api/v1/tournaments/:id/register — register the current user (auth required).
     * Returns: TournamentParticipant
     */
    register: (id) =>
      request(`/api/v1/tournaments/${id}/register`, { method: 'POST' }),

    /**
     * POST /api/v1/tournaments/:id/start — start the tournament (auth required).
     * Generates bracket matches. Returns: Tournament
     */
    start: (id) =>
      request(`/api/v1/tournaments/${id}/start`, { method: 'POST' }),

    /**
     * POST /api/v1/tournaments/:id/match/:matchId/result — submit match result (auth required).
     * data: { winner_id, player1_score?, player2_score? }
     * Returns: TournamentMatch
     */
    submitResult: (tournamentId, matchId, data) =>
      request(`/api/v1/tournaments/${tournamentId}/match/${matchId}/result`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),
  },

  stores: {
    /**
     * List all public stores. params: { game_id?, limit?, offset? }
     * Backend namespace is /api/v1/marketplace/stores (not /api/stores).
     */
    list: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/marketplace/stores${qs ? `?${qs}` : ''}`);
    },
    /** Fetch a single store. */
    get: (storeId) => request(`/api/v1/marketplace/stores/${storeId}`),
    /** Create a developer store. data: { name, game_id, description? } */
    create: (data) => request('/api/v1/marketplace/stores', { method: 'POST', body: JSON.stringify(data) }),
    /** Update store metadata. */
    update: (storeId, data) => request(`/api/v1/marketplace/stores/${storeId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** Delete a store. */
    delete: (storeId) => request(`/api/v1/marketplace/stores/${storeId}`, { method: 'DELETE' }),
    /** List items in a store. */
    items: (storeId) => request(`/api/v1/marketplace/stores/${storeId}/items`),
    /** Add an item to a store. data: { name, description, price_points, price_usd, item_type, metadata? } */
    addItem: (storeId, data) =>
      request(`/api/v1/marketplace/stores/${storeId}/items`, { method: 'POST', body: JSON.stringify(data) }),
    /** Update a store item. */
    updateItem: (storeId, itemId, data) =>
      request(`/api/v1/marketplace/stores/${storeId}/items/${itemId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** Remove an item from a store. */
    removeItem: (storeId, itemId) =>
      request(`/api/v1/marketplace/stores/${storeId}/items/${itemId}`, { method: 'DELETE' }),
    /** Purchase an item. data: { currency: 'points' | 'usd' } */
    purchase: (storeId, itemId, data) =>
      request(`/api/v1/marketplace/stores/${storeId}/items/${itemId}/purchase`, { method: 'POST', body: JSON.stringify(data) }),
    /** Current user's entitlements (purchased items). */
    entitlements: () => request('/api/v1/marketplace/entitlements'),
    /** Developer sales summary for owned stores. */
    sales: (storeId) => request(`/api/v1/marketplace/stores/${storeId}/sales`),
  },
};
