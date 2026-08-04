import { useState, useEffect } from 'react';
import { api } from '../api/client';
import type { AuthUser } from '../types/domain';

export function useUser() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [error] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function fetchUser() {
      try {
        setLoading(true);
        const data = await api.auth.me() as AuthUser;
        if (!cancelled) setUser(data);
      } catch {
        // Not authenticated or network error — leave user as null
        if (!cancelled) setUser(null);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchUser();
    return () => { cancelled = true; };
  }, []);

  return { user, loading, error };
}
