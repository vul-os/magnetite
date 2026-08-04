// usePlayManifest.test.js — real-fetch contract for the play manifest hook.
//
// The hook calls api.distribution.playManifest(gameId) and resolves the live
// ws_endpoint (server_url) that Playground.jsx uses to open the game socket.
// Tests assert the real-fetch path; mock data is gated behind VITE_USE_MOCKS
// which is off by default (and is off in the test environment).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { usePlayManifest, type PlayManifest } from './usePlayManifest';

vi.mock('../api/client', () => ({
  api: {
    distribution: {
      playManifest: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

const mockPlayManifest = vi.mocked(api.distribution.playManifest);

const FAKE_MANIFEST: PlayManifest = {
  game_id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
  version: '1.2.3',
  commit_sha: 'abc123',
  wasm_url: 'https://cdn.example.com/game.wasm',
  server_url: 'wss://runtime.example.com/ws/game/42',
  artifact_type: 'wasm',
  sha256_hash: null,
  file_size_bytes: null,
};

describe('usePlayManifest', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPlayManifest.mockRejectedValue(new Error('No backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── Null gameId ───────────────────────────────────────────────────────────

  it('returns null manifest and no loading when gameId is null', () => {
    const { result } = renderHook(() => usePlayManifest(null));
    expect(result.current.loading).toBe(false);
    expect(result.current.manifest).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('does not call the API when gameId is null', () => {
    renderHook(() => usePlayManifest(null));
    expect(mockPlayManifest).not.toHaveBeenCalled();
  });

  // ── Loading state ─────────────────────────────────────────────────────────

  it('enters loading state immediately when gameId is provided', () => {
    const { result } = renderHook(() => usePlayManifest('game-1'));
    expect(result.current.loading).toBe(true);
  });

  it('calls the API with the correct gameId', async () => {
    mockPlayManifest.mockResolvedValue(FAKE_MANIFEST);
    const { result } = renderHook(() => usePlayManifest('game-42'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));
    expect(mockPlayManifest).toHaveBeenCalledWith('game-42');
  });

  // ── Success — flat response ───────────────────────────────────────────────

  it('populates manifest from a flat API response', async () => {
    mockPlayManifest.mockResolvedValue(FAKE_MANIFEST);

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.manifest).toEqual(FAKE_MANIFEST);
    expect(result.current.error).toBeNull();
  });

  it('populates manifest from a wrapped { data: ... } API response', async () => {
    mockPlayManifest.mockResolvedValue({ data: FAKE_MANIFEST });

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.manifest).toEqual(FAKE_MANIFEST);
    expect(result.current.error).toBeNull();
  });

  it('exposes the live server_url from the manifest', async () => {
    mockPlayManifest.mockResolvedValue(FAKE_MANIFEST);

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.manifest!.server_url).toBe('wss://runtime.example.com/ws/game/42');
  });

  // ── Error state ───────────────────────────────────────────────────────────

  it('sets error and null manifest when API rejects', async () => {
    mockPlayManifest.mockRejectedValue(new Error('Not found'));

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.manifest).toBeNull();
    expect(result.current.error).toBe('Not found');
  });

  it('uses a default error message when API rejects without a message', async () => {
    mockPlayManifest.mockRejectedValue({});

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBeTruthy();
  });

  // ── reload ────────────────────────────────────────────────────────────────

  it('exposes a reload function that re-fetches the manifest', async () => {
    mockPlayManifest.mockResolvedValue(FAKE_MANIFEST);

    const { result } = renderHook(() => usePlayManifest('game-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(typeof result.current.reload).toBe('function');
  });

  // ── Return shape ──────────────────────────────────────────────────────────

  it('exposes the expected shape from the hook', () => {
    const { result } = renderHook(() => usePlayManifest(null));
    expect(typeof result.current.reload).toBe('function');
    expect(typeof result.current.loading).toBe('boolean');
    expect(result.current).toHaveProperty('manifest');
    expect(result.current).toHaveProperty('error');
  });
});
