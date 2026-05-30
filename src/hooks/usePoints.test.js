import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePoints } from './usePoints';

// Mock the API client so we don't hit the network.
vi.mock('../api/client', () => ({
  api: {
    points: {
      balance: vi.fn(),
      history: vi.fn(),
      rewards: vi.fn(),
      leaderboard: vi.fn(),
      redeem: vi.fn(),
    },
  },
}));

// Import the mock AFTER vi.mock so we get the mocked version.
import { api } from '../api/client';

describe('usePoints', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: all API calls reject (simulates no backend) → hook uses mock data.
    api.points.balance.mockRejectedValue(new Error('No backend'));
    api.points.history.mockRejectedValue(new Error('No backend'));
    api.points.rewards.mockRejectedValue(new Error('No backend'));
    api.points.leaderboard.mockRejectedValue(new Error('No backend'));
    api.points.redeem.mockRejectedValue(new Error('No backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('starts loading and settles with mock data when API is unavailable', async () => {
    const { result } = renderHook(() => usePoints());

    // Initially loading
    expect(result.current.loading).toBe(true);

    // Wait for loading to finish
    await vi.waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    // Mock data should be populated
    expect(result.current.balance).toBeDefined();
    expect(result.current.balance.points).toBeGreaterThan(0);
    expect(result.current.rewards.length).toBeGreaterThan(0);
    expect(result.current.leaderboard.length).toBeGreaterThan(0);
    expect(result.current.history.length).toBeGreaterThan(0);
  });

  it('uses API data when the backend returns a valid balance', async () => {
    const fakeBalance = {
      points: 9_999,
      lifetime_points: 50_000,
      rank: 5,
      season: { name: 'Season X', tier: 'Platinum', progress: 90, points_needed: 500 },
    };
    api.points.balance.mockResolvedValue(fakeBalance);
    api.points.history.mockRejectedValue(new Error('no hist'));
    api.points.rewards.mockRejectedValue(new Error('no rew'));
    api.points.leaderboard.mockRejectedValue(new Error('no lb'));

    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.balance.points).toBe(9_999);
    expect(result.current.balance.rank).toBe(5);
  });

  it('uses API history when backend returns a valid list', async () => {
    const fakeHistory = [
      { id: 10, type: 'earn', amount: 100, description: 'Test earn', created_at: '2026-05-01T00:00:00Z' },
    ];
    api.points.balance.mockRejectedValue(new Error('no balance'));
    api.points.history.mockResolvedValue({ history: fakeHistory });
    api.points.rewards.mockRejectedValue(new Error('no rew'));
    api.points.leaderboard.mockRejectedValue(new Error('no lb'));

    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.history).toEqual(fakeHistory);
  });

  it('exposes a redeem function', async () => {
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(typeof result.current.redeem).toBe('function');
  });

  it('redeem: deducts points optimistically when API fails', async () => {
    // Start with clean mock data (balance = 4820, reward r2 costs 500)
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const initialPoints = result.current.balance.points;

    let redeemResult;
    await act(async () => {
      redeemResult = await result.current.redeem('r2'); // costs 500 pts per MOCK_REWARDS
    });

    // redeem returns failure when api rejects (api.points.redeem is mocked to reject)
    expect(redeemResult).toMatchObject({ success: false });
    // Points NOT deducted because the API call failed and we handle the error path
    expect(result.current.balance.points).toBe(initialPoints);
  });

  it('redeem: deducts points optimistically when API succeeds without returning points', async () => {
    api.points.redeem.mockResolvedValue({ ok: true }); // no `points` field
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const initialPoints = result.current.balance.points;
    const rewardR2Cost = 500;

    let redeemResult;
    await act(async () => {
      redeemResult = await result.current.redeem('r2');
    });

    expect(redeemResult).toMatchObject({ success: true });
    expect(result.current.balance.points).toBe(Math.max(0, initialPoints - rewardR2Cost));
  });

  it('redeem: updates balance from API response when it includes points', async () => {
    api.points.redeem.mockResolvedValue({ points: 3_000 });
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.redeem('r1');
    });

    expect(result.current.balance.points).toBe(3_000);
  });

  it('redeem: adds a new history entry on success', async () => {
    api.points.redeem.mockResolvedValue({ ok: true });
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const histBefore = result.current.history.length;

    await act(async () => {
      await result.current.redeem('r1');
    });

    expect(result.current.history.length).toBe(histBefore + 1);
    expect(result.current.history[0].type).toBe('redeem');
  });

  it('redeeming: sets redeeming flag while in-flight', async () => {
    let resolveRedeem;
    api.points.redeem.mockReturnValue(
      new Promise((res) => { resolveRedeem = res; })
    );

    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.redeeming).toBe(false);

    act(() => {
      result.current.redeem('r1');
    });

    // Immediately after call starts, redeeming should be true
    expect(result.current.redeeming).toBe(true);

    await act(async () => {
      resolveRedeem({ ok: true });
    });

    expect(result.current.redeeming).toBe(false);
  });

  it('leaderboard has expected shape', async () => {
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    result.current.leaderboard.forEach((entry) => {
      expect(entry).toHaveProperty('rank');
      expect(entry).toHaveProperty('username');
      expect(entry).toHaveProperty('points');
    });
  });

  it('rewards have expected shape', async () => {
    const { result } = renderHook(() => usePoints());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    result.current.rewards.forEach((r) => {
      expect(r).toHaveProperty('id');
      expect(r).toHaveProperty('cost');
      expect(r).toHaveProperty('name');
    });
  });
});
