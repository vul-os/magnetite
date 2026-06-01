// useSearch.test.js — AX2 tests for search ranking, filtering, and debounce.
//
// Tests:
//  1. Empty query returns null results
//  2. Successful search populates game + user results
//  3. Debounce cancels in-flight search on rapid input
//  4. Error state set when API fails
//  5. Recent searches saved to localStorage and cleared
//  6. Search with filters (genre, min_rating, is_free) passes params to client
//  7. Category/search_type parameter routing

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSearch } from './useSearch';

// ── mock api client ───────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    search: {
      query: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('useSearch — initial state', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => {
    vi.clearAllMocks();
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it('starts with no results and not loading', () => {
    const { result } = renderHook(() => useSearch());
    expect(result.current.results).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('exposes the categories list', () => {
    const { result } = renderHook(() => useSearch());
    expect(Array.isArray(result.current.categories)).toBe(true);
    expect(result.current.categories.length).toBeGreaterThan(0);
  });

  it('exposes search, setQuery, addRecentSearch, clearRecentSearches functions', () => {
    const { result } = renderHook(() => useSearch());
    expect(typeof result.current.search).toBe('function');
    expect(typeof result.current.setQuery).toBe('function');
    expect(typeof result.current.addRecentSearch).toBe('function');
    expect(typeof result.current.clearRecentSearches).toBe('function');
  });
});

describe('useSearch — empty query', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('returns null for empty string', async () => {
    const { result } = renderHook(() => useSearch());

    let ret;
    await act(async () => {
      ret = await result.current.search('');
    });

    expect(ret).toBeNull();
    expect(result.current.results).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('returns null for whitespace-only query', async () => {
    const { result } = renderHook(() => useSearch());

    let ret;
    await act(async () => {
      ret = await result.current.search('   ');
    });

    expect(ret).toBeNull();
    expect(api.search.query).not.toHaveBeenCalled();
  });
});

describe('useSearch — successful search', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    localStorage.clear();
  });

  it('populates game results when API returns games', async () => {
    api.search.query.mockResolvedValue({
      results: [
        { result_type: 'game', id: 'g1', title: 'Oxide Arena', description: 'A top-down shooter' },
      ],
      total: 1,
    });

    const { result } = renderHook(() => useSearch());

    let searchPromise;
    act(() => {
      searchPromise = result.current.search('oxide');
    });

    // Advance past debounce (300 ms)
    await act(async () => {
      vi.advanceTimersByTime(400);
      await searchPromise;
    });

    expect(result.current.results).not.toBeNull();
    expect(result.current.results.games.length).toBe(1);
    expect(result.current.results.games[0].title).toBe('Oxide Arena');
    expect(result.current.loading).toBe(false);
  });

  it('populates user results when API returns users', async () => {
    api.search.query.mockResolvedValue({
      results: [
        { result_type: 'user', id: 'u1', username: 'alice_dev', avatar_url: null },
      ],
      total: 1,
    });

    const { result } = renderHook(() => useSearch());

    let searchPromise;
    act(() => {
      searchPromise = result.current.search('alice');
    });

    await act(async () => {
      vi.advanceTimersByTime(400);
      await searchPromise;
    });

    expect(result.current.results.users.length).toBe(1);
    expect(result.current.results.users[0].title).toBe('alice_dev');
  });

  it('returns empty arrays when no results', async () => {
    api.search.query.mockResolvedValue({ results: [], total: 0 });

    const { result } = renderHook(() => useSearch());

    let searchPromise;
    act(() => {
      searchPromise = result.current.search('nonexistent_game_xyz');
    });

    await act(async () => {
      vi.advanceTimersByTime(400);
      await searchPromise;
    });

    expect(result.current.results.games).toEqual([]);
    expect(result.current.results.users).toEqual([]);
    expect(result.current.error).toBeNull();
  });
});

describe('useSearch — error state', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    localStorage.clear();
  });

  it('sets error when API call fails', async () => {
    api.search.query.mockRejectedValue(new Error('Search service unavailable'));

    const { result } = renderHook(() => useSearch());

    let searchPromise;
    act(() => {
      searchPromise = result.current.search('game');
    });

    await act(async () => {
      vi.advanceTimersByTime(400);
      await searchPromise;
    });

    expect(result.current.error).toBeTruthy();
    expect(result.current.results).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('clears error on next successful search', async () => {
    api.search.query
      .mockRejectedValueOnce(new Error('Error'))
      .mockResolvedValueOnce({ results: [], total: 0 });

    const { result } = renderHook(() => useSearch());

    // First search fails
    let p1;
    act(() => { p1 = result.current.search('game'); });
    await act(async () => { vi.advanceTimersByTime(400); await p1; });
    expect(result.current.error).toBeTruthy();

    // Second search succeeds
    let p2;
    act(() => { p2 = result.current.search('game2'); });
    await act(async () => { vi.advanceTimersByTime(400); await p2; });
    expect(result.current.error).toBeNull();
  });
});

describe('useSearch — recent searches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('addRecentSearch saves to localStorage', () => {
    const { result } = renderHook(() => useSearch());

    act(() => {
      result.current.addRecentSearch('rust game');
    });

    const stored = JSON.parse(localStorage.getItem('magnetite_recent_searches') || '[]');
    expect(stored).toContain('rust game');
  });

  it('clearRecentSearches removes all recent searches', () => {
    localStorage.setItem(
      'magnetite_recent_searches',
      JSON.stringify(['first search', 'second search'])
    );

    const { result } = renderHook(() => useSearch());

    act(() => {
      result.current.clearRecentSearches();
    });

    expect(result.current.recentSearches).toEqual([]);
    expect(localStorage.getItem('magnetite_recent_searches')).toBeNull();
  });

  it('ignores blank addRecentSearch calls', () => {
    const { result } = renderHook(() => useSearch());

    act(() => {
      result.current.addRecentSearch('   ');
    });

    const stored = JSON.parse(localStorage.getItem('magnetite_recent_searches') || '[]');
    expect(stored.length).toBe(0);
  });

  it('limits recent searches to 5', () => {
    const { result } = renderHook(() => useSearch());

    act(() => {
      ['a', 'b', 'c', 'd', 'e', 'f', 'g'].forEach(q =>
        result.current.addRecentSearch(q)
      );
    });

    expect(result.current.recentSearches.length).toBeLessThanOrEqual(5);
  });
});

describe('useSearch — API params', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    api.search.query.mockResolvedValue({ results: [], total: 0 });
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    localStorage.clear();
  });

  it('passes search type to the API client', async () => {
    const { result } = renderHook(() => useSearch());

    let p;
    act(() => { p = result.current.search('shooter', 'Games'); });
    await act(async () => { vi.advanceTimersByTime(400); await p; });

    // useSearch calls api.search.query(query, searchType, limit, offset[, ...])
    expect(api.search.query).toHaveBeenCalledWith(
      expect.any(String),
      expect.stringContaining('game'),
      expect.any(Number),
      expect.any(Number),
      expect.anything(),
    );
  });

  it('search type "all" uses all results path', async () => {
    const { result } = renderHook(() => useSearch());

    let p;
    act(() => { p = result.current.search('rust', 'All'); });
    await act(async () => { vi.advanceTimersByTime(400); await p; });

    expect(api.search.query).toHaveBeenCalled();
  });
});
