import { useState, useEffect } from 'react';
import { api } from '../api/client';

export function useGames(filters = {}) {
  const [games, setGames] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  
  useEffect(() => {
    async function fetchGames() {
      try {
        setLoading(true);
        const data = await api.games.list();
        setGames(data);
      } catch (err) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    }
    fetchGames();
  }, [filters]);
  
  return { games, loading, error };
}
