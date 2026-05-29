import { useState, useEffect } from 'react';
import { api } from '../api/client';
import { mockGames } from '../data/mockGames';

export function useGames(_filters = {}) {
  const [games, setGames] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    let cancelled = false;

    async function fetchGames() {
      try {
        setLoading(true);
        setError(null);
        const data = await api.games.list();
        if (!cancelled) {
          setGames(Array.isArray(data) ? data : (data?.games ?? mockGames));
        }
      } catch (err) {
        if (!cancelled) {
          setError(err.message);
          setGames(mockGames); // graceful fallback
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchGames();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { games, loading, error };
}
