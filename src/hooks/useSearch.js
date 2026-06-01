import { useState, useCallback, useRef, useEffect } from 'react';
import { api } from '../api/client';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';
const RECENT_SEARCHES_KEY = 'magnetite_recent_searches';
const MAX_RECENT_SEARCHES = 5;
const DEBOUNCE_MS = 300;

function getRecentSearches() {
  try {
    return JSON.parse(localStorage.getItem(RECENT_SEARCHES_KEY)) || [];
  } catch {
    return [];
  }
}

function saveRecentSearch(query) {
  const recent = getRecentSearches();
  const filtered = recent.filter(s => s.toLowerCase() !== query.toLowerCase());
  const updated = [query, ...filtered].slice(0, MAX_RECENT_SEARCHES);
  localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(updated));
}

function clearRecentSearches() {
  localStorage.removeItem(RECENT_SEARCHES_KEY);
}

const CATEGORIES = ['All', 'Games', 'Users', 'Leaderboard', 'Achievements'];

const GENRES = ['Action', 'Adventure', 'Puzzle', 'Racing', 'RPG', 'Shooter', 'Strategy', 'Simulation', 'Sports', 'Other'];

async function fetchSearchResults(query, searchType = 'all', filters = {}) {
  if (USE_MOCKS) {
    const q = query.toLowerCase();
    const { mockGames }       = await import('../data/mockGames');
    const { mockSearchUsers } = await import('../data/mockFriends');
    const results = { games: [], users: [] };
    if (searchType === 'All' || searchType === 'Games') {
      results.games = mockGames
        .filter(g => g.title.toLowerCase().includes(q) || (g.developer ?? '').toLowerCase().includes(q))
        .filter(g => !filters.genre || (g.genre ?? '').toLowerCase() === filters.genre.toLowerCase())
        .slice(0, 5)
        .map(g => ({ type: 'game', id: g.id, title: g.title, subtitle: g.developer ?? '', ...g }));
    }
    if (searchType === 'All' || searchType === 'Users') {
      results.users = mockSearchUsers
        .filter(u => u.username.toLowerCase().includes(q))
        .slice(0, 5)
        .map(u => ({ type: 'user', id: u.id, title: u.username, subtitle: u.status ?? '', ...u }));
    }
    return results;
  }

  // Real API path — let errors propagate so callers can show an error state
  const data = await api.search.query(query, searchType.toLowerCase(), 20, 0, filters);
  return {
    games: (data.results ?? [])
      .filter(r => r.result_type === 'game')
      .map(g => ({
        type: 'game',
        id: g.id,
        title: g.title,
        subtitle: g.description || '',
        result_type: 'game',
        genre: g.genre ?? null,
        tags: g.tags ?? [],
      })),
    users: (data.results ?? [])
      .filter(r => r.result_type === 'user')
      .map(u => ({
        type: 'user',
        id: u.id,
        title: u.username,
        subtitle: u.avatar_url || '',
        result_type: 'user',
      })),
  };
}

export function useSearch() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState(null);
  const [loading, setLoading] = useState(false);
  const [recentSearches, setRecentSearches] = useState(getRecentSearches);
  const debounceRef = useRef(null);
  const [filters, setFilters] = useState({});

  const [searchError, setSearchError] = useState(null);

  const search = useCallback(async (searchQuery, category = 'All', activeFilters = {}) => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    if (!searchQuery.trim()) {
      setResults(null);
      setLoading(false);
      setSearchError(null);
      return null;
    }

    setLoading(true);
    setSearchError(null);

    return new Promise((resolve) => {
      debounceRef.current = setTimeout(async () => {
        try {
          const data = await fetchSearchResults(searchQuery, category, activeFilters);
          setResults(data);
          setLoading(false);
          resolve(data);
        } catch (err) {
          setResults(null);
          setSearchError(err.message ?? 'Search failed');
          setLoading(false);
          resolve(null);
        }
      }, DEBOUNCE_MS);
    });
  }, []);

  const addRecentSearch = useCallback((searchQuery) => {
    if (!searchQuery.trim()) return;
    saveRecentSearch(searchQuery);
    setRecentSearches(getRecentSearches());
  }, []);

  const clearRecentSearchesFn = useCallback(() => {
    clearRecentSearches();
    setRecentSearches([]);
  }, []);

  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, []);

  return {
    query,
    setQuery,
    results,
    loading,
    error: searchError,
    search,
    filters,
    setFilters,
    recentSearches,
    addRecentSearch,
    clearRecentSearches: clearRecentSearchesFn,
    categories: CATEGORIES,
    genres: GENRES,
  };
}
