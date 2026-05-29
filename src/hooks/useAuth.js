import { useState, useEffect, useCallback } from 'react';

const TOKEN_KEY = 'magnetite_token';
const USER_KEY = 'magnetite_user';

export function useAuth() {
  const [user, setUser] = useState(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const token = localStorage.getItem(TOKEN_KEY);
    const storedUser = localStorage.getItem(USER_KEY);
    if (token && storedUser) {
      setUser(JSON.parse(storedUser));
    }
    setIsLoading(false);
  }, []);

  const login = useCallback(async (email, password) => {
    await new Promise((r) => setTimeout(r, 500));
    if (!email || !password) {
      throw new Error('Invalid credentials');
    }
    const mockUser = { id: 1, username: email.split('@')[0], email };
    const token = 'mock_jwt_token_' + Date.now();
    localStorage.setItem(TOKEN_KEY, token);
    localStorage.setItem(USER_KEY, JSON.stringify(mockUser));
    setUser(mockUser);
  }, []);

  const register = useCallback(async (username, email, password) => {
    await new Promise((r) => setTimeout(r, 500));
    if (!username || !email || !password) {
      throw new Error('All fields are required');
    }
    const mockUser = { id: 1, username, email };
    const token = 'mock_jwt_token_' + Date.now();
    localStorage.setItem(TOKEN_KEY, token);
    localStorage.setItem(USER_KEY, JSON.stringify(mockUser));
    setUser(mockUser);
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    setUser(null);
  }, []);

  return { user, login, register, logout, isLoading };
}
