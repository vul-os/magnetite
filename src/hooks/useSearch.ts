import { useState, useCallback, useRef, useEffect } from 'react';
import { api } from '../api/client';
import type { SearchResultItem, SearchResults } from '../types/domain';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';
const RECENT_SEARCHES_KEY = 'magnetite_recent_searches';
const MAX_RECENT_SEARCHES = 5;
const DEBOUNCE_MS = 300;

export interface SearchFilters {
  genre?: string;
  [key: string]: unknown;
}

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

function getRecentSearches(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_SEARCHES_KEY) || 'null') || [];
  } catch {
    return [];
  }
}

function saveRecentSearch(query: string) {
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

interface SearchApiResult {
  id: string | number;
  result_type: 'game' | 'user';
  title?: string;
  username?: string;
  description?: string;
  avatar_url?: string;
  genre?: string;
  tags?: string[];
  [key: string]: unknown;
}

async function fetchSearchResults(query: string, searchType = 'all', filters: SearchFilters = {}): Promise<SearchResults> {
  if (USE_MOCKS) {
    const q = query.toLowerCase();
    const { mockGames }       = await import('../data/mockGames');
    const { mockSearchUsers } = await import('../data/mockFriends');
    const results: SearchResults = { games: [], users: [] };
    if (searchType === 'All' || searchType === 'Games') {
      results.games = mockGames
        .filter(g => g.title.toLowerCase().includes(q) || (g.developer ?? '').toLowerCase().includes(q))
        .filter(g => !filters.genre || (g.genre ?? '').toLowerCase() === filters.genre.toLowerCase())
        .slice(0, 5)
        .map(g => ({ ...g, type: 'game' as const, id: g.id, title: g.title, subtitle: g.developer ?? '' }));
    }
    if (searchType === 'All' || searchType === 'Users') {
      results.users = mockSearchUsers
        .filter(u => u.username.toLowerCase().includes(q))
        .slice(0, 5)
        .map(u => ({ ...u, type: 'user' as const, id: u.id, title: u.username, subtitle: u.status ?? '' }));
    }
    return results;
  }

  // Real API path — let errors propagate so callers can show an error state
  const data = await api.search.query(query, searchType.toLowerCase(), 20, 0, filters) as { results?: SearchApiResult[] };
  const apiResults = data.results ?? [];
  return {
    games: apiResults
      .filter(r => r.result_type === 'game')
      .map((g): SearchResultItem => ({
        type: 'game',
        id: g.id,
        title: g.title ?? '',
        subtitle: g.description || '',
        result_type: 'game',
        genre: g.genre ?? null,
        tags: g.tags ?? [],
      })),
    users: apiResults
      .filter(r => r.result_type === 'user')
      .map((u): SearchResultItem => ({
        type: 'user',
        id: u.id,
        title: u.username ?? '',
        subtitle: u.avatar_url || '',
        result_type: 'user',
      })),
  };
}

export function useSearch() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [recentSearches, setRecentSearches] = useState<string[]>(getRecentSearches);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [filters, setFilters] = useState<SearchFilters>({});

  const [searchError, setSearchError] = useState<string | null>(null);

  const search = useCallback((searchQuery: string, category = 'All', activeFilters: SearchFilters = {}): Promise<SearchResults | null> => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    if (!searchQuery.trim()) {
      setResults(null);
      setLoading(false);
      setSearchError(null);
      return Promise.resolve(null);
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
          setSearchError(errMessage(err, 'Search failed'));
          setLoading(false);
          resolve(null);
        }
      }, DEBOUNCE_MS);
    });
  }, []);

  const addRecentSearch = useCallback((searchQuery: string) => {
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
