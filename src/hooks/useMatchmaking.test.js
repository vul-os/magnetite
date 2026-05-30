// useMatchmaking.test.js — tests for the gap-closure real-fetch contract.
// useMatchmaking now calls the real API; these tests mock the api client
// and verify the hook's loading/error/data states, the real-API path,
// and the wait-estimate propagation.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useMatchmaking } from './useMatchmaking';

vi.mock('../api/client', () => ({
  api: {
    matchmaking: {
      join: vi.fn(),
      leave: vi.fn(),
      status: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

const GAME_ID = 'game-abc-123';

describe('useMatchmaking', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.matchmaking.join.mockRejectedValue(new Error('No backend'));
    api.matchmaking.leave.mockRejectedValue(new Error('No backend'));
    api.matchmaking.status.mockRejectedValue(new Error('No backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── Initial state ─────────────────────────────────────────────────────────

  it('starts with null status and no match data', () => {
    const { result } = renderHook(() => useMatchmaking());
    expect(result.current.status).toBeNull();
    expect(result.current.matchData).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  // ── joinQueue ─────────────────────────────────────────────────────────────

  it('joinQueue: calls the real API with the game ID', async () => {
    api.matchmaking.join.mockResolvedValue({
      in_queue: true,
      status: 'waiting',
      estimated_wait_seconds: 30,
    });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(api.matchmaking.join).toHaveBeenCalledWith(GAME_ID);
  });

  it('joinQueue: sets status to searching while in-flight', async () => {
    let resolveJoin;
    api.matchmaking.join.mockReturnValue(new Promise((r) => { resolveJoin = r; }));

    const { result } = renderHook(() => useMatchmaking());

    act(() => {
      result.current.joinQueue(GAME_ID);
    });

    expect(result.current.status).toBe('searching');
    expect(result.current.loading).toBe(true);

    await act(async () => {
      resolveJoin({ in_queue: true, status: 'waiting', estimated_wait_seconds: 60 });
    });

    expect(result.current.loading).toBe(false);
  });

  it('joinQueue: populates matchData from API response', async () => {
    const fakeResponse = {
      in_queue: true,
      queue_id: 'q-001',
      game_id: GAME_ID,
      status: 'waiting',
      estimated_wait_seconds: 45,
    };
    api.matchmaking.join.mockResolvedValue(fakeResponse);

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(result.current.matchData).toEqual(fakeResponse);
  });

  it('joinQueue: sets status from API response', async () => {
    api.matchmaking.join.mockResolvedValue({ status: 'waiting', estimated_wait_seconds: 30 });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(result.current.status).toBe('waiting');
  });

  it('joinQueue: sets status to searching when API response has no status field', async () => {
    api.matchmaking.join.mockResolvedValue({ in_queue: true });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(result.current.status).toBe('searching');
  });

  it('joinQueue: sets error and status="error" when API fails', async () => {
    api.matchmaking.join.mockRejectedValue(new Error('Queue full'));

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(result.current.status).toBe('error');
    expect(result.current.error).toBe('Queue full');
  });

  // ── leaveQueue ────────────────────────────────────────────────────────────

  it('leaveQueue: calls the real API', async () => {
    api.matchmaking.leave.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.leaveQueue();
    });

    expect(api.matchmaking.leave).toHaveBeenCalled();
  });

  it('leaveQueue: resets status and matchData on success', async () => {
    // First join so state is non-null.
    api.matchmaking.join.mockResolvedValue({ status: 'waiting' });
    api.matchmaking.leave.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });
    expect(result.current.status).toBe('waiting');

    await act(async () => {
      await result.current.leaveQueue();
    });

    expect(result.current.status).toBeNull();
    expect(result.current.matchData).toBeNull();
  });

  it('leaveQueue: silently ignores API errors', async () => {
    api.matchmaking.leave.mockRejectedValue(new Error('Leave failed'));

    const { result } = renderHook(() => useMatchmaking());

    // Should not throw
    await act(async () => {
      await result.current.leaveQueue();
    });

    // loading should be reset to false even on error
    expect(result.current.loading).toBe(false);
  });

  // ── pollStatus ────────────────────────────────────────────────────────────

  it('pollStatus: updates matchData from status API', async () => {
    const fakeStatus = {
      in_queue: true,
      status: 'waiting',
      estimated_wait_seconds: 90,
    };
    api.matchmaking.status.mockResolvedValue(fakeStatus);

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.pollStatus();
    });

    expect(result.current.matchData).toEqual(fakeStatus);
  });

  it('pollStatus: silently ignores errors', async () => {
    api.matchmaking.status.mockRejectedValue(new Error('Poll failed'));

    const { result } = renderHook(() => useMatchmaking());

    // Should not throw or set error
    await act(async () => {
      await result.current.pollStatus();
    });

    expect(result.current.error).toBeNull();
  });

  // ── Wait estimate ─────────────────────────────────────────────────────────

  it('matchData carries estimated_wait_seconds from server', async () => {
    api.matchmaking.join.mockResolvedValue({
      status: 'waiting',
      estimated_wait_seconds: 150,
    });

    const { result } = renderHook(() => useMatchmaking());

    await act(async () => {
      await result.current.joinQueue(GAME_ID);
    });

    expect(result.current.matchData.estimated_wait_seconds).toBe(150);
  });

  it('wait estimate is not fabricated Math.random (server-sourced)', async () => {
    // The old code used Math.random()*20+5 for queue depth.
    // Now the wait estimate must come from the API response.
    api.matchmaking.join.mockResolvedValue({
      status: 'waiting',
      estimated_wait_seconds: 30,
    });

    const { result: r1 } = renderHook(() => useMatchmaking());
    await act(async () => { await r1.current.joinQueue(GAME_ID); });

    api.matchmaking.join.mockResolvedValue({
      status: 'waiting',
      estimated_wait_seconds: 30,
    });

    const { result: r2 } = renderHook(() => useMatchmaking());
    await act(async () => { await r2.current.joinQueue(GAME_ID); });

    // Both should get exactly the mocked server value, not a random number.
    expect(r1.current.matchData.estimated_wait_seconds).toBe(30);
    expect(r2.current.matchData.estimated_wait_seconds).toBe(30);
  });

  // ── Return shape ──────────────────────────────────────────────────────────

  it('exposes the expected surface', () => {
    const { result } = renderHook(() => useMatchmaking());
    expect(typeof result.current.joinQueue).toBe('function');
    expect(typeof result.current.leaveQueue).toBe('function');
    expect(typeof result.current.pollStatus).toBe('function');
    expect('status' in result.current).toBe(true);
    expect('matchData' in result.current).toBe(true);
    expect('loading' in result.current).toBe(true);
    expect('error' in result.current).toBe(true);
  });
});
