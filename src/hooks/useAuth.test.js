import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAuth } from '../hooks/useAuth';

// Mock the API client so we don't hit the network.
vi.mock('../api/client', () => ({
  api: {
    auth: {
      login: vi.fn(),
      me: vi.fn(),
    },
  },
}));

// Import the mock AFTER vi.mock so we get the mocked version.
import { api } from '../api/client';

describe('useAuth', () => {
  const TOKEN_KEY = 'magnetite_token';
  const USER_KEY = 'magnetite_user';

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    // Default: api.auth.me rejects (no backend), so restore falls back to stored user.
    api.auth.me.mockRejectedValue(new Error('No backend'));
    // Default: api.auth.login resolves with real token + user.
    api.auth.login.mockResolvedValue({
      token: 'real_token_123',
      user: { id: 1, email: 'test@example.com', username: 'testuser' },
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('login function stores token and user', async () => {
    api.auth.login.mockResolvedValue({
      token: 'real_token_123',
      user: { id: 1, email: 'test@example.com', username: 'testuser' },
    });

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await result.current.login('test@example.com', 'password123');
    });

    expect(localStorage.getItem(TOKEN_KEY)).toBe('real_token_123');
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
    api.auth.login.mockResolvedValue({
      token: 'real_token_abc',
      user: { id: 2, email: 'user@test.com', username: 'user' },
    });

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await result.current.login('user@test.com', 'pass123');
    });

    const token = localStorage.getItem(TOKEN_KEY);
    // The hook stores the real token returned by the API — not a fabricated mock token.
    expect(token).toBe('real_token_abc');

    act(() => {
      result.current.logout();
    });

    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
  });

  it('login throws error with invalid credentials', async () => {
    api.auth.login.mockRejectedValue(new Error('Invalid credentials'));

    const { result } = renderHook(() => useAuth());

    let caughtError;
    await act(async () => {
      try {
        await result.current.login('', '');
      } catch (e) {
        caughtError = e;
      }
    });

    expect(caughtError).toBeDefined();
    expect(caughtError.message).toBe('Invalid credentials');
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
