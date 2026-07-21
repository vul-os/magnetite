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
    const err = new Error(error.message || 'Request failed');
    err.status = response.status;
    // A 404 on a route the backend never mounted is not the same fact as a
    // request that failed (DESIGN.md §7.2). Callers use this to choose between
    // <LoadError> and <Unavailable>.
    err.notFound = response.status === 404;
    throw err;
  }

  // Several backend handlers answer 204 No Content (moderation actions, some
  // deletes). Parsing an empty body as JSON throws and would surface a real
  // success as a failure.
  if (response.status === 204 || response.headers.get('content-length') === '0') {
    return null;
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
    /** GET /api/v1/profile/me — the authenticated user's own profile */
    me: () => request('/api/v1/profile/me'),
    /** PUT /api/v1/profile/me — update the authenticated user's profile.
     *  The backend nests profile::router() under /api/v1/profile and the
     *  authed handlers live at /me; calling /api/v1/profile bare 404s, which
     *  is what silently broke every profile save. */
    update: (data) => request('/api/v1/profile/me', { method: 'PUT', body: JSON.stringify(data) }),
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
    /** GET /api/v1/friends/blocked — users this account has blocked. */
    blocked: () => request('/api/v1/friends/blocked'),
    /** POST /api/v1/friends/block/:id */
    blockUser: (userId) => request(`/api/v1/friends/block/${userId}`, { method: 'POST' }),
    /** DELETE /api/v1/friends/block/:id */
    unblockUser: (userId) => request(`/api/v1/friends/block/${userId}`, { method: 'DELETE' }),
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
     * POST /api/v1/admin/review-reports/:id/action — act on a reported review.
     * data: { action: 'dismiss' | 'remove_review' | 'warn_user' | 'ban_user', note? }
     * Answers 204 No Content.
     */
    actOnReport: (reportId, data) =>
      request(`/api/v1/admin/review-reports/${reportId}/action`, { method: 'POST', body: JSON.stringify(data) }),
    /** @deprecated alias of actOnReport — the backend route is /action, not /dismiss. */
    dismissReport: (reportId, data) =>
      request(`/api/v1/admin/review-reports/${reportId}/action`, { method: 'POST', body: JSON.stringify(data) }),

    /**
     * GET /api/v1/admin/chat-flags — auto-flagged chat messages.
     * params: { page?, limit?, status? }
     */
    chatFlags: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/admin/chat-flags${qs ? `?${qs}` : ''}`);
    },
    /**
     * POST /api/v1/admin/chat-flags/:id/action
     * data: { action: 'dismiss' | 'warn_user' | 'ban_user', note? }
     */
    actOnChatFlag: (flagId, data) =>
      request(`/api/v1/admin/chat-flags/${flagId}/action`, { method: 'POST', body: JSON.stringify(data) }),
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
    /**
     * PUT /api/v1/admin/users/:id/ban — set a user's ban state.
     * The backend route is PUT and takes { banned: bool, reason? }; both ban and
     * unban go through it.
     */
    banUser: (userId, reason) =>
      request(`/api/v1/admin/users/${userId}/ban`, { method: 'PUT', body: JSON.stringify({ banned: true, reason }) }),
    /** PUT /api/v1/admin/users/:id/ban with banned:false. */
    unbanUser: (userId) =>
      request(`/api/v1/admin/users/${userId}/ban`, { method: 'PUT', body: JSON.stringify({ banned: false }) }),

    // ── Not mounted on this node ────────────────────────────────────────────
    // The three calls below have no corresponding backend route. They are kept
    // so callers keep their shape, but they will 404 — a caller must surface
    // <Unavailable>, not <LoadError>. See src/components/state/Unavailable.jsx.

    /** UNAVAILABLE — no POST /admin/transactions/:id/refund route exists. */
    refundTransaction: (transactionId, data = {}) =>
      request(`/api/v1/admin/transactions/${transactionId}/refund`, { method: 'POST', body: JSON.stringify(data) }),
    /**
     * UNAVAILABLE — no POST /admin/users/:id/warn route exists.
     * To warn the author of a reported review, use
     * actOnReport(reportId, { action: 'warn_user' }), which IS implemented.
     */
    warnUser: (userId, reason) =>
      request(`/api/v1/admin/users/${userId}/warn`, { method: 'POST', body: JSON.stringify({ reason }) }),
    /** UNAVAILABLE — no GET /admin/review-reports/:id detail route exists. */
    getReport: (reportId) => request(`/api/v1/admin/review-reports/${reportId}`),
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

    /**
     * GET /api/v1/developer/games/:gameId/versions — the registered versions of
     * a game, newest first. Each: { id, game_id, version, commit_sha,
     * release_notes, is_live, created_at, updated_at }.
     *
     * This is the real deployment history. There is no separate "builds"
     * collection on the backend — a version IS the deployable unit.
     */
    versions: (gameId) => request(`/api/v1/developer/games/${gameId}/versions`),

    /**
     * GET /api/v1/developer/games/:gameId/build-status — the build-status
     * summary for a game's artifacts.
     */
    buildStatus: (gameId) => request(`/api/v1/developer/games/${gameId}/build-status`),

    // ── GDS: Game scaffold ───────────────────────────────────────────────────
    /**
     * POST /api/v1/developer/games/scaffold
     * body: { name: string, template_id: string, description?: string }
     * Returns: { game_id, name, template_id, created_at, cli_instructions: string, next_steps: string[] }
     */
    scaffold: (data) =>
      request('/api/v1/developer/games/scaffold', { method: 'POST', body: JSON.stringify(data) }),

    /**
     * UNAVAILABLE — there is no build-log store or route on this node. Nothing
     * on the backend persists CI output, so there is nothing to fetch; the page
     * must say so rather than show an empty log pane.
     */
    buildLogs: (gameId, buildId) => request(`/api/v1/developer/games/${gameId}/builds/${buildId}/logs`),

    /**
     * PUT /api/v1/developer/games/:gameId/versions/:versionId/promote — make a
     * version live (ownership-checked).
     */
    promote: (gameId, versionId) =>
      request(`/api/v1/developer/games/${gameId}/versions/${versionId}/promote`, { method: 'PUT' }),

    /**
     * PUT /api/v1/developer/games/:gameId/versions/:versionId/rollback —
     * demote the current live version and promote this one.
     */
    rollback: (gameId, versionId) =>
      request(`/api/v1/developer/games/${gameId}/versions/${versionId}/rollback`, { method: 'PUT' }),

    // Payout recipients are GONE. Developers are paid wallet-to-wallet at
    // checkout (seam §3.6), so there is no bank account to register, no
    // recipient to maintain and no payout to schedule. The developer's
    // destination is simply the wallet address linked via `api.wallet.link`.
    // (There is no requestPayout/payoutStatus call any more — /api/v1/developer/payouts
    // does not exist server-side; GET /api/v1/developer/earnings is the real,
    // receipt-backed record of what a developer was paid.)
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
    /** Leave a community. The backend mounts POST /:id/leave, not DELETE. */
    leave: (id) => request(`/api/communities/${id}/leave`, { method: 'POST' }),
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
    /**
     * List DM messages between the current user and another.
     * Backend route is GET /dms/:other_user_id/messages — GET /dms/:id is not
     * mounted (only POST is, to send).
     */
    listDMs: (userId, params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/dms/${userId}/messages${qs ? `?${qs}` : ''}`);
    },
    /** GET /api/v1/dms — the current user's DM threads. */
    dmThreads: () => request('/api/dms'),
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
     * communityId = 'global' → platform-wide listing (GET /api/v1/streams)
     * communityId = specific id → GET /api/v1/communities/:id/streams
     *
     * There is no /streams/live route; GET /streams already lists only live
     * streams. The old code called /streams/live and swallowed the 404 into a
     * silent fallback, which hid the mistake.
     */
    list: (communityId) =>
      communityId === 'global'
        ? request('/api/v1/streams')
        : request(`/api/v1/communities/${communityId}/streams`),

    /** Start streaming — community-scoped when given a community. */
    goLive: (communityId, data) =>
      communityId && communityId !== 'global'
        ? request(`/api/v1/communities/${communityId}/streams`, { method: 'POST', body: JSON.stringify(data) })
        : request('/api/v1/streams', { method: 'POST', body: JSON.stringify(data) }),

    /**
     * Stop a stream. The backend mounts POST /streams/:id/stop; there is no
     * DELETE /streams/:id — a stream is ended, not deleted.
     */
    end: (streamId) => request(`/api/v1/streams/${streamId}/stop`, { method: 'POST' }),

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
    /**
     * UNAVAILABLE — there is no rewards catalogue on this node. The points
     * router mounts balance / leaderboard / season / award / spend / history
     * only; no /points/rewards and no /points/redeem.
     */
    rewards: () => request('/api/points/rewards'),
    /** UNAVAILABLE — no POST /points/redeem route exists. */
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
     * UNAVAILABLE — GET /api/v1/replays is not mounted. The replays router
     * owns POST / (store) and GET /:id (fetch one) only. Replays can be listed
     * per game via listForGame() below, which IS mounted.
     *
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
     * GET /api/v1/games/:gameId/replays — replays for one game. This is the
     * only replay listing the backend mounts.
     */
    listForGame: (gameId, params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/games/${gameId}/replays${qs ? `?${qs}` : ''}`);
    },

    /** UNAVAILABLE — there is no DELETE /api/v1/replays/:id route. */
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

  /**
   * Developer stores. Backend namespace is /api/v1/marketplace/* (mirrored at
   * /api/v1/stores/* by `marketplace::stores_router`). Each entry below records
   * whether the route is actually mounted — several are not, and the page must
   * say so rather than fail silently.
   */
  stores: {
    /**
     * The stores owned by the current developer.
     * GET /api/v1/marketplace/my-stores — mounted.
     * (There is no public "list every store" route; the old `list()` called
     * GET /marketplace/stores, which 404s.)
     */
    mine: () => request('/api/v1/marketplace/my-stores'),
    /** UNAVAILABLE — no GET /marketplace/stores route. Use mine(). */
    list: (params = {}) => {
      const qs = new URLSearchParams(
        Object.fromEntries(Object.entries(params).filter(([, v]) => v != null))
      ).toString();
      return request(`/api/v1/marketplace/stores${qs ? `?${qs}` : ''}`);
    },
    /** GET /api/v1/marketplace/stores/:game_id — the store for a game. Mounted. */
    get: (storeId) => request(`/api/v1/marketplace/stores/${storeId}`),
    /**
     * Create a store for a game. The backend mounts
     * POST /marketplace/games/:game_id/store — a store belongs to a game, so
     * the game id is in the path, not the body.
     * data: { name, description? }
     */
    create: (gameId, data) =>
      request(`/api/v1/marketplace/games/${gameId}/store`, { method: 'POST', body: JSON.stringify(data) }),
    /** PUT /api/v1/marketplace/stores/:store_id — mounted. */
    update: (storeId, data) => request(`/api/v1/marketplace/stores/${storeId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** UNAVAILABLE — no DELETE /marketplace/stores/:id route exists. */
    delete: (storeId) => request(`/api/v1/marketplace/stores/${storeId}`, { method: 'DELETE' }),
    /** GET /api/v1/marketplace/stores/:store_id/items — mounted. */
    items: (storeId) => request(`/api/v1/marketplace/stores/${storeId}/items`),
    /** POST /api/v1/marketplace/stores/:store_id/items — mounted. */
    addItem: (storeId, data) =>
      request(`/api/v1/marketplace/stores/${storeId}/items`, { method: 'POST', body: JSON.stringify(data) }),
    /** PUT /api/v1/marketplace/items/:item_id — items are addressed globally. */
    updateItem: (storeId, itemId, data) =>
      request(`/api/v1/marketplace/items/${itemId}`, { method: 'PUT', body: JSON.stringify(data) }),
    /** UNAVAILABLE — no delete-item route exists on the backend. */
    removeItem: (storeId, itemId) =>
      request(`/api/v1/marketplace/items/${itemId}`, { method: 'DELETE' }),
    /** POST /api/v1/marketplace/items/:item_id/purchase — mounted. */
    purchase: (storeId, itemId, data) =>
      request(`/api/v1/marketplace/items/${itemId}/purchase`, { method: 'POST', body: JSON.stringify(data) }),
    /** GET /api/v1/marketplace/entitlements — mounted. */
    entitlements: () => request('/api/v1/marketplace/entitlements'),
    /**
     * Revenue for a store. Backend route is /revenue, not /sales.
     * GET /api/v1/marketplace/stores/:store_id/revenue — mounted.
     */
    revenue: (storeId) => request(`/api/v1/marketplace/stores/${storeId}/revenue`),
    /** @deprecated alias of revenue(). */
    sales: (storeId) => request(`/api/v1/marketplace/stores/${storeId}/revenue`),
  },
};
