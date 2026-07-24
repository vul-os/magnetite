import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// Single canonical token key shared across all auth modules and client.js.
// client.js (not owned by this agent) reads localStorage.getItem('token') —
// keeping the same key here eliminates the former magnetite_token/token split.
const TOKEN_KEY = 'token';
const USER_KEY = 'magnetite_user';

// Mock mode (VITE_USE_MOCKS=true) runs with no backend — every data context
// short-circuits its network calls (see NotificationContext, WalletContext).
// Session restore must do the same, or it fires /auth/me at a server that is
// not running and floods the console with 503s on every mock/screenshot boot.
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

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
      // Under mocks there is no API to validate against; trust the stored user
      // (as the catch branch below already does when the API is unreachable)
      // rather than firing a doomed /auth/me request.
      if (USE_MOCKS) {
        if (storedUser) {
          try { setUser(JSON.parse(storedUser)); } catch { /* invalid JSON */ }
        }
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
    }
    localStorage.setItem(USER_KEY, JSON.stringify(userData));
    setUser(userData);
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    setUser(null);
  }, []);

  return { user, login, register, logout, isLoading };
}
