import { useState, useEffect } from 'react';
import { api } from '../api/client';
import { mockGames } from '../data/mockGames';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export function useGames(_filters = {}) {
  const [games, setGames] = useState(USE_MOCKS ? mockGames : []);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function fetchGames() {
      setLoading(true);
      setError(null);
      try {
        const data = await api.games.list();
        if (!cancelled) {
          // Accept either { games: [...] } or a plain array
          const list = Array.isArray(data) ? data : (data?.games ?? data?.data ?? []);
          setGames(list);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err.message || 'Failed to load games');
          setGames([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchGames();
    return () => { cancelled = true; };
  }, []);

  return { games, loading, error };
}
