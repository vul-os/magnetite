import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

const TOKEN_KEY = 'magnetite_token';
const USER_KEY = 'magnetite_user';

export function useAuth() {
  const [user, setUser] = useState(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    async function restoreSession() {
      const token = localStorage.getItem(TOKEN_KEY);
      const storedUser = localStorage.getItem(USER_KEY);
      if (!token) {
        setIsLoading(false);
        return;
      }
      // Try to validate token against real API
      try {
        const me = await api.auth.me();
        setUser(me);
        localStorage.setItem(USER_KEY, JSON.stringify(me));
      } catch {
        // Fall back to stored user if API unavailable
        if (storedUser) {
          try { setUser(JSON.parse(storedUser)); } catch { /* invalid JSON */ }
        } else {
          // Token invalid — clear it
          localStorage.removeItem(TOKEN_KEY);
        }
      } finally {
        setIsLoading(false);
      }
    }
    restoreSession();
  }, []);

  const login = useCallback(async (email, password) => {
    const result = await api.auth.login({ email, password });
    const token = result.token || result.access_token;
    const userData = result.user || result;
    if (token) {
      localStorage.setItem(TOKEN_KEY, token);
      // Also set under the key used by client.js
      localStorage.setItem('token', token);
    }
    localStorage.setItem(USER_KEY, JSON.stringify(userData));
    setUser(userData);
  }, []);

  const register = useCallback(async (username, email, password) => {
    const result = await api.auth.register({ username, email, password });
    const token = result.token || result.access_token;
    const userData = result.user || result;
    if (token) {
      localStorage.setItem(TOKEN_KEY, token);
      localStorage.setItem('token', token);
    }
    localStorage.setItem(USER_KEY, JSON.stringify(userData));
    setUser(userData);
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    localStorage.removeItem('token');
    setUser(null);
  }, []);

  return { user, login, register, logout, isLoading };
}
