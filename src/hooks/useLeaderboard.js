import { useState, useEffect } from 'react';
import { api } from '../api/client';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export function useLeaderboard(gameId) {
  const [entries, setEntries] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (!gameId) {
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
            setEntries(mockLeaderboard[key] || mockLeaderboard['1'] || []);
          }
          return;
        }

        const data = await api.games.leaderboard(gameId);
        if (!cancelled) {
          setEntries(Array.isArray(data) ? data : (data?.entries ?? []));
        }
      } catch (err) {
        if (!cancelled) {
          setError(err.message ?? 'Failed to load leaderboard');
          setEntries([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchLeaderboard();
    return () => { cancelled = true; };
  }, [gameId]);

  return { entries, loading, error };
}
