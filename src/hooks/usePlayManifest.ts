// usePlayManifest — fetches the play manifest from the distribution API.
//
// Endpoint: GET /api/v1/distribution/:game_id/play
// Response fields used by the play flow:
//   server_url  — live WebSocket URL the browser should connect to
//   wasm_url    — optional client-side WASM bundle URL
//   version     — semver string of the live version
//   commit_sha  — HEAD commit SHA
//
// Mock: when VITE_USE_MOCKS=true returns a deterministic stub so the play
// pages can be developed without a running backend.

import { useState, useEffect } from 'react';
import { api } from '../api/client';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export interface PlayManifest {
  game_id: string;
  version: string;
  commit_sha: string;
  wasm_url: string | null;
  server_url: string;
  artifact_type: string;
  sha256_hash: string | null;
  file_size_bytes: number | null;
}

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

// ── Mock data — only used when VITE_USE_MOCKS=true ───────────────────────────
function buildMockManifest(gameId: string): PlayManifest {
  return {
    game_id: gameId,
    version: '0.1.0-mock',
    commit_sha: 'deadbeefdeadbeef',
    wasm_url: null,
    server_url: `ws://localhost:9000/ws/game/${gameId}`,
    artifact_type: 'wasm',
    sha256_hash: null,
    file_size_bytes: null,
  };
}

/**
 * Fetch the play manifest for a game.
 *
 * @param gameId  UUID of the game.
 */
export function usePlayManifest(gameId: string | null | undefined) {
  const [manifest, setManifest] = useState<PlayManifest | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rev, setRev] = useState(0);

  const reload = () => setRev((r) => r + 1);

  // Loads the play manifest for a game from the API (external system); resetting
  // state when gameId clears is part of that synchronization.
  useEffect(() => {
    if (!gameId) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setManifest(null);
      setLoading(false);
      setError(null);
      return;
    }

    let cancelled = false;

    if (USE_MOCKS) {
      setLoading(false);
      setManifest(buildMockManifest(gameId));
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    (async () => {
      try {
        // The backend wraps responses in { data: { ... } } via the success_response helper.
        const body = await api.distribution.playManifest(gameId);
        if (cancelled) return;
        // Unwrap optional { data: ... } wrapper
        const m = (body as { data?: PlayManifest } | null)?.data ?? (body as PlayManifest | null);
        setManifest(m);
        setError(null);
      } catch (err) {
        if (cancelled) return;
        setManifest(null);
        setError(errMessage(err, 'Failed to load play manifest'));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => { cancelled = true; };
    // `rev` is intentionally included so callers can force a refresh via reload().
  }, [gameId, rev]);

  return { manifest, loading, error, reload };
}
