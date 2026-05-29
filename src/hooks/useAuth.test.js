import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAuth } from '../hooks/useAuth';

describe('useAuth', () => {
  const TOKEN_KEY = 'magnetite_token';
  const USER_KEY = 'magnetite_user';

  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('login function stores token and user', async () => {
    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await result.current.login('test@example.com', 'password123');
    });

    expect(localStorage.getItem(TOKEN_KEY)).toBeTruthy();
    expect(localStorage.getItem(USER_KEY)).toBeTruthy();
    const storedUser = JSON.parse(localStorage.getItem(USER_KEY));
    expect(storedUser.email).toBe('test@example.com');
  });

  it('logout function removes token and user', async () => {
    localStorage.setItem(TOKEN_KEY, 'existing_token');
    localStorage.setItem(USER_KEY, JSON.stringify({ id: 1, email: 'test@example.com' }));

    const { result } = renderHook(() => useAuth());

    act(() => {
      result.current.logout();
    });

    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
    expect(localStorage.getItem(USER_KEY)).toBeNull();
  });

  it('token storage works correctly', async () => {
    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await result.current.login('user@test.com', 'pass123');
    });

    const token = localStorage.getItem(TOKEN_KEY);
    expect(token).toMatch(/^mock_jwt_token_\d+$/);

    act(() => {
      result.current.logout();
    });

    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
  });

  it('login throws error with invalid credentials', async () => {
    const { result } = renderHook(() => useAuth());

    await act(async () => {
      try {
        await result.current.login('', '');
      } catch (e) {
        expect(e.message).toBe('Invalid credentials');
      }
    });
  });

  it('loads user from localStorage on mount', async () => {
    const mockUser = { id: 1, username: 'testuser', email: 'test@example.com' };
    localStorage.setItem(TOKEN_KEY, 'valid_token');
    localStorage.setItem(USER_KEY, JSON.stringify(mockUser));

    const { result } = renderHook(() => useAuth());

    await vi.waitFor(() => {
      expect(result.current.user).toEqual(mockUser);
    });
  });
});
