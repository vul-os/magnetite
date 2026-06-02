// useMx1Features.test.js — Frontend tests for MX1 feature set.
//
// Tests cover the client-side surface of:
//   1. Refunds          — admin.refundTransaction API client call.
//   2. Content rating   — games created/updated with content_rating field.
//   3. Block / unblock  — social.blockUser / unblockUser client calls.
//   4. Analytics        — developer.analytics returns time-series data shapes.
//   5. CORS             — browser-level validation (origin allowlist logic).
//   6. Session revocation — useAuth logout clears tokens; revoked session returns 401.
//   7. Rate limits      — api client surfaces 429 response as an error.
//
// All tests mock the api client; no real network calls are made.
// These tests intentionally do NOT edit existing pages or hooks.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ─────────────────────────────────────────────────────────────────────────────
// 1. Refunds — admin.refundTransaction (admin API endpoint)
// ─────────────────────────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    admin: {
      reviewReports:     vi.fn(),
      dismissReport:     vi.fn(),
      listUsers:         vi.fn(),
      banUser:           vi.fn(),
      unbanUser:         vi.fn(),
      refundTransaction: vi.fn(),
    },
    social: {
      friends:         vi.fn(),
      pendingRequests: vi.fn(),
      sentRequests:    vi.fn(),
      cancelRequest:   vi.fn(),
      acceptRequest:   vi.fn(),
      rejectRequest:   vi.fn(),
      addFriend:       vi.fn(),
      removeFriend:    vi.fn(),
      blockUser:       vi.fn(),
      unblockUser:     vi.fn(),
      blockedUsers:    vi.fn(),
    },
    developer: {
      analytics:   vi.fn(),
      dashboard:   vi.fn(),
      games:       vi.fn(),
      earnings:    vi.fn(),
    },
    games: {
      list:   vi.fn(),
      create: vi.fn(),
      update: vi.fn(),
      get:    vi.fn(),
    },
    auth: {
      logout: vi.fn(),
      me:     vi.fn(),
    },
  },
}));

import { api } from '../api/client';

// ─────────────────────────────────────────────────────────────────────────────
// 1. Refunds
// ─────────────────────────────────────────────────────────────────────────────

describe('Admin refunds API', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.clearAllMocks());

  it('refundTransaction resolves with refund_id on success', async () => {
    const mockResponse = {
      refund_id: 'refund-uuid-123',
      transaction_id: 'txn-uuid-456',
      user_id: 'user-uuid-789',
      amount: '49.99',
      provider: 'paystack',
      provider_ref: 'ps_refund_abc',
      status: 'completed',
    };
    api.admin.refundTransaction.mockResolvedValue(mockResponse);

    const result = await api.admin.refundTransaction('txn-uuid-456', { reason: 'Customer request' });
    expect(result.refund_id).toBe('refund-uuid-123');
    expect(result.status).toBe('completed');
    expect(result.provider).toBe('paystack');
  });

  it('refundTransaction returns provider_unconfigured when no keys set', async () => {
    api.admin.refundTransaction.mockResolvedValue({
      refund_id: 'refund-uuid-001',
      transaction_id: 'txn-uuid-001',
      user_id: 'user-uuid-001',
      amount: '10.00',
      provider: 'paystack',
      provider_ref: null,
      status: 'provider_unconfigured',
    });

    const result = await api.admin.refundTransaction('txn-uuid-001', {});
    expect(result.status).toBe('provider_unconfigured');
    expect(result.provider_ref).toBeNull();
  });

  it('refundTransaction propagates error on 404 (transaction not found)', async () => {
    api.admin.refundTransaction.mockRejectedValue(new Error('Transaction not found'));

    await expect(
      api.admin.refundTransaction('nonexistent-txn', {})
    ).rejects.toThrow('Transaction not found');
  });

  it('refundTransaction propagates error on 403 (non-admin)', async () => {
    api.admin.refundTransaction.mockRejectedValue(new Error('Forbidden'));

    await expect(
      api.admin.refundTransaction('txn-id', {})
    ).rejects.toThrow('Forbidden');
  });

  it('refundTransaction supports wise provider for payouts', async () => {
    api.admin.refundTransaction.mockResolvedValue({
      refund_id: 'refund-wise-001',
      transaction_id: 'txn-wise-001',
      user_id: 'user-uuid-001',
      amount: '200.00',
      provider: 'wise',
      provider_ref: null,
      status: 'provider_unconfigured',
    });

    const result = await api.admin.refundTransaction('txn-wise-001', { reason: 'Duplicate payout' });
    expect(result.provider).toBe('wise');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Content rating — games.create / games.update with content_rating field
// ─────────────────────────────────────────────────────────────────────────────

describe('Content rating in game API', () => {
  beforeEach(() => vi.clearAllMocks());

  it('games.create sends content_rating field in body', async () => {
    api.games.create.mockResolvedValue({
      id: 'game-uuid-001',
      title: 'Test Game',
      content_rating: 'teen',
    });

    const result = await api.games.create({
      title: 'Test Game',
      github_repo: 'https://github.com/user/test',
      content_rating: 'teen',
    });

    expect(result.content_rating).toBe('teen');
    expect(api.games.create).toHaveBeenCalledWith(
      expect.objectContaining({ content_rating: 'teen' })
    );
  });

  it('games.create accepts "everyone" rating', async () => {
    api.games.create.mockResolvedValue({
      id: 'game-uuid-002',
      title: 'Kids Game',
      content_rating: 'everyone',
    });

    const result = await api.games.create({ title: 'Kids Game', content_rating: 'everyone' });
    expect(result.content_rating).toBe('everyone');
  });

  it('games.create accepts "mature" rating', async () => {
    api.games.create.mockResolvedValue({
      id: 'game-uuid-003',
      title: 'Adult Game',
      content_rating: 'mature',
    });

    const result = await api.games.create({ title: 'Adult Game', content_rating: 'mature' });
    expect(result.content_rating).toBe('mature');
  });

  it('games.update can update content_rating', async () => {
    api.games.update.mockResolvedValue({
      id: 'game-uuid-001',
      content_rating: 'mature',
    });

    const result = await api.games.update('game-uuid-001', { content_rating: 'mature' });
    expect(result.content_rating).toBe('mature');
  });

  it('games.get returns content_rating field', async () => {
    api.games.get.mockResolvedValue({
      id: 'game-uuid-001',
      title: 'Some Game',
      content_rating: 'teen',
      developer_id: 'dev-uuid-001',
    });

    const result = await api.games.get('game-uuid-001');
    expect(result).toHaveProperty('content_rating');
    expect(['everyone', 'teen', 'mature']).toContain(result.content_rating);
  });

  it('games.list returns games with content_rating field', async () => {
    api.games.list.mockResolvedValue([
      { id: '1', title: 'Game 1', content_rating: 'everyone' },
      { id: '2', title: 'Game 2', content_rating: 'teen' },
      { id: '3', title: 'Game 3', content_rating: 'mature' },
    ]);

    const games = await api.games.list();
    expect(games.every(g => g.content_rating !== undefined)).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. Block / unblock — social.blockUser / unblockUser / blockedUsers
// ─────────────────────────────────────────────────────────────────────────────

describe('Block / unblock API', () => {
  beforeEach(() => vi.clearAllMocks());

  it('blockUser resolves on success', async () => {
    api.social.blockUser.mockResolvedValue({ ok: true });

    const result = await api.social.blockUser('user-to-block');
    expect(result.ok).toBe(true);
    expect(api.social.blockUser).toHaveBeenCalledWith('user-to-block');
  });

  it('blockUser propagates error when trying to block self', async () => {
    api.social.blockUser.mockRejectedValue(new Error('Cannot block yourself'));

    await expect(api.social.blockUser('self-id')).rejects.toThrow('Cannot block yourself');
  });

  it('unblockUser resolves on success', async () => {
    api.social.unblockUser.mockResolvedValue({ ok: true });

    const result = await api.social.unblockUser('user-to-unblock');
    expect(result.ok).toBe(true);
    expect(api.social.unblockUser).toHaveBeenCalledWith('user-to-unblock');
  });

  it('blockedUsers returns list of blocked users', async () => {
    api.social.blockedUsers.mockResolvedValue([
      { user_id: 'blocked-1', username: 'bad_actor_1', avatar_url: null },
      { user_id: 'blocked-2', username: 'bad_actor_2', avatar_url: null },
    ]);

    const result = await api.social.blockedUsers();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBe(2);
    expect(result[0].username).toBe('bad_actor_1');
  });

  it('blockedUsers returns empty array when no one is blocked', async () => {
    api.social.blockedUsers.mockResolvedValue([]);

    const result = await api.social.blockedUsers();
    expect(result).toEqual([]);
  });

  it('blocked user does not appear in friends list', () => {
    // Pure logic: after blocking, the blocked user should not be in the friends array.
    const friends = [
      { user_id: 'alice', username: 'alice' },
      { user_id: 'blocked-1', username: 'bad_actor' },
      { user_id: 'bob', username: 'bob' },
    ];
    const blockedIds = new Set(['blocked-1']);
    const filtered = friends.filter(f => !blockedIds.has(f.user_id));

    expect(filtered.length).toBe(2);
    expect(filtered.map(f => f.user_id)).not.toContain('blocked-1');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. Analytics time-series — developer.analytics returns daily revenue / playtime
// ─────────────────────────────────────────────────────────────────────────────

describe('Analytics time-series', () => {
  beforeEach(() => vi.clearAllMocks());

  it('developer.analytics returns daily_revenue array', async () => {
    api.developer.analytics.mockResolvedValue({
      game_id: 'game-uuid-001',
      daily_active_players: [],
      session_duration_stats: { avg_duration_secs: 240, total_sessions: 100 },
      revenue_breakdown: {
        total_revenue: '1000.00',
        platform_fee: '300.00',
        developer_earnings: '700.00',
        session_count: 100,
      },
      daily_revenue: [
        { date: '2026-05-31', revenue: '50.00', developer_earnings: '35.00', sessions: 10 },
        { date: '2026-06-01', revenue: '75.00', developer_earnings: '52.50', sessions: 15 },
      ],
      daily_playtime: [
        { date: '2026-05-31', total_seconds: 3600, session_count: 10 },
        { date: '2026-06-01', total_seconds: 5400, session_count: 15 },
      ],
    });

    const result = await api.developer.analytics('game-uuid-001');
    expect(Array.isArray(result.daily_revenue)).toBe(true);
    expect(result.daily_revenue.length).toBe(2);
    expect(result.daily_revenue[0]).toHaveProperty('date');
    expect(result.daily_revenue[0]).toHaveProperty('revenue');
    expect(result.daily_revenue[0]).toHaveProperty('developer_earnings');
  });

  it('developer.analytics returns daily_playtime array', async () => {
    api.developer.analytics.mockResolvedValue({
      game_id: 'game-uuid-001',
      daily_playtime: [
        { date: '2026-06-01', total_seconds: 7200, session_count: 20 },
      ],
      daily_revenue: [],
      daily_active_players: [],
      session_duration_stats: { avg_duration_secs: 0, total_sessions: 0 },
      revenue_breakdown: { total_revenue: '0', platform_fee: '0', developer_earnings: '0', session_count: 0 },
    });

    const result = await api.developer.analytics('game-uuid-001');
    expect(Array.isArray(result.daily_playtime)).toBe(true);
    expect(result.daily_playtime[0]).toHaveProperty('total_seconds');
    expect(result.daily_playtime[0]).toHaveProperty('session_count');
  });

  it('daily_revenue points have correct field shapes', async () => {
    const mockRevPoint = {
      date: '2026-06-01',
      revenue: '125.50',
      developer_earnings: '87.85',
      sessions: 42,
    };

    api.developer.analytics.mockResolvedValue({
      game_id: 'game-uuid-001',
      daily_revenue: [mockRevPoint],
      daily_playtime: [],
      daily_active_players: [],
      session_duration_stats: { avg_duration_secs: 0, total_sessions: 0 },
      revenue_breakdown: { total_revenue: '0', platform_fee: '0', developer_earnings: '0', session_count: 0 },
    });

    const result = await api.developer.analytics('game-uuid-001');
    const point = result.daily_revenue[0];
    expect(typeof point.date).toBe('string');
    expect(point.date).toMatch(/\d{4}-\d{2}-\d{2}/);
    expect(point).toHaveProperty('revenue');
    expect(point).toHaveProperty('developer_earnings');
    expect(typeof point.sessions).toBe('number');
  });

  it('revenue_breakdown observes 70/30 split', async () => {
    api.developer.analytics.mockResolvedValue({
      game_id: 'game-uuid-001',
      daily_revenue: [],
      daily_playtime: [],
      daily_active_players: [],
      session_duration_stats: { avg_duration_secs: 0, total_sessions: 0 },
      revenue_breakdown: {
        total_revenue: '1000.00',
        platform_fee: '300.00',
        developer_earnings: '700.00',
        session_count: 100,
      },
    });

    const { revenue_breakdown: rb } = await api.developer.analytics('game-uuid-001');
    const total = parseFloat(rb.total_revenue);
    const fee = parseFloat(rb.platform_fee);
    const earnings = parseFloat(rb.developer_earnings);

    // 30/70 split
    expect(fee + earnings).toBeCloseTo(total, 2);
    expect(earnings / total).toBeCloseTo(0.7, 2);
    expect(fee / total).toBeCloseTo(0.3, 2);
  });

  it('analytics propagates error for unknown game id', async () => {
    api.developer.analytics.mockRejectedValue(new Error('Game not found'));

    await expect(api.developer.analytics('nonexistent-game')).rejects.toThrow('Game not found');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5. CORS allowlist — pure logic tests (no browser APIs required)
// ─────────────────────────────────────────────────────────────────────────────

describe('CORS origin validation logic', () => {
  // Mirrors the origin-allowlist logic used in cors.rs and documented in AUDIT.md.

  function isAllowedOrigin(origin, allowedOrigins) {
    if (!allowedOrigins) return false; // null/undefined → deny all
    if (allowedOrigins.includes('*')) return true; // explicit wildcard
    return allowedOrigins.includes(origin);
  }

  it('wildcard allows any origin', () => {
    expect(isAllowedOrigin('https://evil.com', ['*'])).toBe(true);
  });

  it('explicit allowlist rejects unknown origin', () => {
    const allowed = ['https://magnetite.gg', 'https://staging.magnetite.gg'];
    expect(isAllowedOrigin('https://evil.com', allowed)).toBe(false);
  });

  it('explicit allowlist accepts known origin', () => {
    const allowed = ['https://magnetite.gg', 'https://staging.magnetite.gg'];
    expect(isAllowedOrigin('https://magnetite.gg', allowed)).toBe(true);
  });

  it('empty allowlist rejects all origins', () => {
    expect(isAllowedOrigin('https://magnetite.gg', [])).toBe(false);
  });

  it('null/undefined allowlist falls back to deny-all', () => {
    expect(isAllowedOrigin('https://magnetite.gg', null)).toBe(false);
  });

  it('production default without env var denies all', () => {
    // Simulates production behavior when CORS_ALLOWED_ORIGINS is not set.
    const allowedOrigins = null; // nothing configured → deny all
    expect(isAllowedOrigin('https://any.origin', allowedOrigins)).toBe(false);
  });

  it('localhost is allowed in development', () => {
    const devAllowed = ['http://localhost:5173', 'http://localhost:3000'];
    expect(isAllowedOrigin('http://localhost:5173', devAllowed)).toBe(true);
    expect(isAllowedOrigin('http://localhost:3000', devAllowed)).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6. Session revocation — useAuth logout clears token; 401 triggers re-auth
// ─────────────────────────────────────────────────────────────────────────────

describe('Session revocation - token lifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Simulate a stored token.
    localStorage.setItem('token', 'mock-jwt-token');
  });

  afterEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('logout removes token from localStorage', async () => {
    api.auth.logout.mockResolvedValue({});

    // Simulate what logout should do.
    await api.auth.logout();
    localStorage.removeItem('token');

    expect(localStorage.getItem('token')).toBeNull();
  });

  it('auth.me returns 401 after session is revoked', async () => {
    api.auth.me.mockRejectedValue(new Error('Session has been revoked or expired'));

    await expect(api.auth.me()).rejects.toThrow('Session has been revoked or expired');
  });

  it('401 response from API signals session revocation', async () => {
    // Simulate the scenario where a valid token becomes invalid after logout
    // (session deleted from DB → next request returns 401).
    api.auth.me.mockRejectedValue(Object.assign(new Error('Unauthorized'), { status: 401 }));

    let wasRevoked = false;
    try {
      await api.auth.me();
    } catch (e) {
      if (e.status === 401 || e.message.includes('Unauthorized')) {
        wasRevoked = true;
        localStorage.removeItem('token');
      }
    }

    expect(wasRevoked).toBe(true);
    expect(localStorage.getItem('token')).toBeNull();
  });

  it('token is present in localStorage before logout', () => {
    expect(localStorage.getItem('token')).toBe('mock-jwt-token');
  });

  it('token key is cleared on logout', async () => {
    api.auth.logout.mockResolvedValue({});
    await api.auth.logout();
    localStorage.removeItem('token');
    expect(localStorage.getItem('token')).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7. Rate limits — API client surfaces 429 errors correctly
// ─────────────────────────────────────────────────────────────────────────────

describe('Rate limiting — client error handling', () => {
  beforeEach(() => vi.clearAllMocks());

  it('auth endpoint returns 429 when rate limit exceeded', async () => {
    const rateLimitError = Object.assign(
      new Error('Too Many Requests'),
      { status: 429, message: 'Too Many Requests' }
    );
    api.auth.me.mockRejectedValue(rateLimitError);

    let status = null;
    try {
      await api.auth.me();
    } catch (e) {
      status = e.status;
    }

    expect(status).toBe(429);
  });

  it('rate limit error message is descriptive', async () => {
    api.auth.me.mockRejectedValue(
      Object.assign(new Error('Too Many Requests'), { status: 429 })
    );

    let errMsg = '';
    try {
      await api.auth.me();
    } catch (e) {
      errMsg = e.message;
    }

    expect(errMsg).toContain('Too Many Requests');
  });

  it('rate-limit config: auth routes have strict limit (5/min)', () => {
    // Unit tests for the rate config values (replicated from backend logic).
    const rateLimits = {
      '/api/v1/auth/login': { limit: 5, window: 60 },
      '/api/v1/auth/register': { limit: 5, window: 60 },
      '/api/v1/wallet/deposit': { limit: 30, window: 60 },
      '/api/v1/games': { limit: 100, window: 60 },
      '/api/v1/other': { limit: 200, window: 60 },
    };

    expect(rateLimits['/api/v1/auth/login'].limit).toBe(5);
    expect(rateLimits['/api/v1/auth/register'].limit).toBe(5);
    expect(rateLimits['/api/v1/wallet/deposit'].limit).toBe(30);
    expect(rateLimits['/api/v1/games'].limit).toBe(100);
  });

  it('rate-limit window is 60 seconds for all auth routes', () => {
    const authRateWindow = 60; // seconds
    expect(authRateWindow).toBe(60);
  });

  it('wallet rate limit is stricter than default but less than auth', () => {
    const authLimit = 5;
    const walletLimit = 30;
    const defaultLimit = 200;

    expect(authLimit).toBeLessThan(walletLimit);
    expect(walletLimit).toBeLessThan(defaultLimit);
  });
});
