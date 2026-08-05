import { useState, useEffect } from 'react';
import { api } from '../api/client';
import { mockGames } from '../data/mockGames';
import type { Game } from '../types/domain';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

export function useGames(_filters: Record<string, unknown> = {}) {
  const [games, setGames] = useState<Game[]>(USE_MOCKS ? mockGames : []);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [error, setError] = useState<string | null>(null);

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
          const body = data as { games?: Game[]; data?: Game[] } | Game[] | null;
          const list = Array.isArray(body) ? body : (body?.games ?? body?.data ?? []);
          setGames(list);
        }
      } catch (err) {
        if (!cancelled) {
          setError(errMessage(err, 'Failed to load games'));
          setGames([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void fetchGames();
    return () => { cancelled = true; };
  }, []);

  return { games, loading, error };
}
