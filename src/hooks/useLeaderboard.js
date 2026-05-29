import { useState, useEffect } from 'react';
import { api } from '../api/client';
import { mockLeaderboard } from '../data/mockLeaderboard';

export function useLeaderboard(gameId) {
  const [entries, setEntries] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error] = useState(null);

  useEffect(() => {
    if (!gameId) {
      setLoading(false);
      return;
    }

    let cancelled = false;

    async function fetchLeaderboard() {
      try {
        setLoading(true);
        const data = await api.games.leaderboard(gameId);
        if (!cancelled) {
          setEntries(Array.isArray(data) ? data : (data?.entries ?? []));
        }
      } catch {
        if (!cancelled) {
          // Fall back to mock data
          const key = String(gameId);
          setEntries(mockLeaderboard[key] || mockLeaderboard['1'] || []);
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
