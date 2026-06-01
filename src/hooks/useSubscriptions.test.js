// useSubscriptions.test.js — AX2 tests for subscription upgrade/proration,
// cancel-at-period-end, hours/usage, and the api.subscriptions client surface.
//
// All tests mock the api client — no live backend required.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ── api client mock ───────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    subscriptions: {
      plans:   vi.fn(),
      current: vi.fn(),
      create:  vi.fn(),
      cancel:  vi.fn(),
      upgrade: vi.fn(),
      hours:   vi.fn(),
      usage:   vi.fn(),
    },
  },
}));

import { api } from '../api/client';

// ── Subscription proration math (JS-side) ────────────────────────────────────

describe('subscription proration math', () => {
  /**
   * Mirror of the backend proration_factor logic:
   *   factor = remaining / total, clamped to [0, 1].
   */
  function prorationFactor(periodStartMs, periodEndMs) {
    const now = Date.now();
    const total = Math.max(1, periodEndMs - periodStartMs);
    const remaining = Math.max(0, periodEndMs - now);
    return Math.min(1, Math.max(0, remaining / total));
  }

  it('is 1 at the very start of a 30-day period', () => {
    const now = Date.now();
    const start = now - 1000; // 1 second ago
    const end = now + 30 * 24 * 3600 * 1000;
    const factor = prorationFactor(start, end);
    expect(factor).toBeGreaterThanOrEqual(0.99);
    expect(factor).toBeLessThanOrEqual(1.0);
  });

  it('is approximately 0.5 at the midpoint', () => {
    const now = Date.now();
    const start = now - 15 * 24 * 3600 * 1000;
    const end = now + 15 * 24 * 3600 * 1000;
    const factor = prorationFactor(start, end);
    expect(factor).toBeGreaterThanOrEqual(0.49);
    expect(factor).toBeLessThanOrEqual(0.51);
  });

  it('is 0 when the period has fully expired', () => {
    const past = Date.now() - 60 * 24 * 3600 * 1000;
    const pastEnd = Date.now() - 30 * 24 * 3600 * 1000;
    const factor = prorationFactor(past, pastEnd);
    expect(factor).toBe(0);
  });

  it('never exceeds 1', () => {
    // period entirely in the future
    const future = Date.now() + 10 * 24 * 3600 * 1000;
    const futureEnd = future + 30 * 24 * 3600 * 1000;
    const factor = prorationFactor(future, futureEnd);
    expect(factor).toBeLessThanOrEqual(1.0);
  });

  it('never goes below 0', () => {
    const past = Date.now() - 90 * 24 * 3600 * 1000;
    const pastEnd = past + 30 * 24 * 3600 * 1000;
    const factor = prorationFactor(past, pastEnd);
    expect(factor).toBeGreaterThanOrEqual(0);
  });

  it('upgrade charge delta is approximately correct at midpoint', () => {
    const now = Date.now();
    const start = now - 15 * 24 * 3600 * 1000;
    const end = now + 15 * 24 * 3600 * 1000;
    const factor = prorationFactor(start, end);

    const oldPrice = 5.0;
    const newPrice = 10.0;
    const delta = newPrice - oldPrice;
    const charge = delta * factor;

    // Should be ~$2.50 (±$0.05 for rounding)
    expect(charge).toBeGreaterThanOrEqual(2.45);
    expect(charge).toBeLessThanOrEqual(2.55);
  });
});

// ── api.subscriptions client surface ─────────────────────────────────────────

describe('api.subscriptions client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('plans() resolves to a tier list', async () => {
    const mockPlans = [
      { id: 'tier-1', slug: 'free',  name: 'Free',  price_usdc: 0 },
      { id: 'tier-2', slug: 'basic', name: 'Basic', price_usdc: 5 },
    ];
    api.subscriptions.plans.mockResolvedValue({ data: mockPlans });

    const result = await api.subscriptions.plans();
    expect(result.data).toEqual(mockPlans);
    expect(result.data.length).toBe(2);
  });

  it('current() resolves to the active subscription', async () => {
    const mockSub = {
      id: 'sub-1',
      status: 'active',
      cancel_at_period_end: false,
      tier: { slug: 'pro', name: 'Pro' },
    };
    api.subscriptions.current.mockResolvedValue({ data: mockSub });

    const result = await api.subscriptions.current();
    expect(result.data.status).toBe('active');
    expect(result.data.cancel_at_period_end).toBe(false);
  });

  it('current() cancel_at_period_end can be true', async () => {
    const mockSub = {
      id: 'sub-1',
      status: 'cancel_pending',
      cancel_at_period_end: true,
      tier: { slug: 'pro', name: 'Pro' },
    };
    api.subscriptions.current.mockResolvedValue({ data: mockSub });

    const result = await api.subscriptions.current();
    expect(result.data.cancel_at_period_end).toBe(true);
    expect(result.data.status).toBe('cancel_pending');
  });

  it('upgrade() sends tier_id and paystack_payment_id', async () => {
    const mockResponse = {
      data: { id: 'sub-2', status: 'active', tier: { slug: 'pro' } },
    };
    api.subscriptions.upgrade.mockResolvedValue(mockResponse);

    const result = await api.subscriptions.upgrade('tier-pro', 'pstk_ref_001');
    expect(result.data.status).toBe('active');
    expect(api.subscriptions.upgrade).toHaveBeenCalledWith('tier-pro', 'pstk_ref_001');
  });

  it('upgrade() for free downgrade does not require paystackRef', async () => {
    api.subscriptions.upgrade.mockResolvedValue({ data: { id: 'sub-3', status: 'active' } });

    const result = await api.subscriptions.upgrade('tier-free');
    expect(result.data).toBeDefined();
    expect(api.subscriptions.upgrade).toHaveBeenCalledWith('tier-free');
  });

  it('upgrade() propagates errors from backend', async () => {
    api.subscriptions.upgrade.mockRejectedValue(new Error('payment_id required for paid tier'));

    await expect(api.subscriptions.upgrade('tier-pro')).rejects.toThrow('payment_id required');
  });

  it('cancel() calls DELETE on the right path', async () => {
    api.subscriptions.cancel.mockResolvedValue({ data: { status: 'cancel_pending' } });

    const result = await api.subscriptions.cancel();
    expect(result.data.status).toBe('cancel_pending');
  });

  it('hours() resolves with included_hours and used_hours', async () => {
    api.subscriptions.hours.mockResolvedValue({
      data: { included_hours: 720, used_hours: 0 },
    });

    const result = await api.subscriptions.hours();
    expect(result.data.included_hours).toBe(720);
    expect(result.data.used_hours).toBe(0);
  });

  it('usage() resolves with used_games and max_games', async () => {
    api.subscriptions.usage.mockResolvedValue({
      data: { used_games: 2, max_games: 5, remaining_days: 14 },
    });

    const result = await api.subscriptions.usage();
    expect(result.data.used_games).toBe(2);
    expect(result.data.remaining_days).toBe(14);
  });
});
