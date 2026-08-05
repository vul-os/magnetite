import { useState, useEffect } from 'react';
import { api } from '../api/client';
import type { LeaderboardEntry } from '../types/domain';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

export function useLeaderboard(gameId: string | number | null | undefined) {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Fetches leaderboard data (external API); loading/reset state is necessarily
  // driven from within the effect.
  useEffect(() => {
    if (!gameId) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setLoading(false);
      return;
    }

    let cancelled = false;

    async function fetchLeaderboard() {
      setError(null);
      try {
        setLoading(true);

        if (USE_MOCKS) {
          const { mockLeaderboard } = await import('../data/mockLeaderboard');
          if (!cancelled) {
            const key = String(gameId);
            const table = mockLeaderboard as Record<string, LeaderboardEntry[]>;
            setEntries(table[key] || table['1'] || []);
          }
          return;
        }

        const data = await api.games.leaderboard(gameId as string | number);
        if (!cancelled) {
          const body = data as { entries?: LeaderboardEntry[] } | LeaderboardEntry[] | null;
          setEntries(Array.isArray(body) ? body : (body?.entries ?? []));
        }
      } catch (err) {
        if (!cancelled) {
          setError(errMessage(err, 'Failed to load leaderboard'));
          setEntries([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void fetchLeaderboard();
    return () => { cancelled = true; };
  }, [gameId]);

  return { entries, loading, error };
}
