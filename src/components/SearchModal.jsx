import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useSearch } from '../hooks/useSearch';
import Spinner from './common/Spinner';
import './SearchModal.css';

const CATEGORY_FILTERS = ['All', 'Games', 'Users'];

export function SearchModal({ isOpen, onClose }) {
  const navigate = useNavigate();
  const { query, setQuery, search, recentSearches, addRecentSearch, clearRecentSearches, loading, results, filters, setFilters, genres } = useSearch();
  const [selectedCategory, setSelectedCategory] = useState('All');
  const [activeIndex, setActiveIndex] = useState(-1);
  const [showFilters, setShowFilters] = useState(false);
  const inputRef = useRef(null);
  const modalRef = useRef(null);
  const cleanupRef = useRef(false);

  const flatResults = useMemo(() => {
    if (!results) return [];
    return [
      ...(results.games || []).map(g => ({ ...g, category: 'game' })),
      ...(results.users || []).map(u => ({ ...u, category: 'user' })),
    ];
  }, [results]);

  const groupedResults = useMemo(() => {
    if (!results) return { games: [], users: [] };
    return {
      games: results.games || [],
      users: results.users || [],
    };
  }, [results]);

  useEffect(() => {
    const handleKeyDown = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        if (!isOpen) {
          document.getElementById('search-modal-input')?.focus();
        }
      }
      if (e.key === '/' && isOpen && document.activeElement !== inputRef.current) {
        e.preventDefault();
        inputRef.current?.focus();
      }
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus();
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
      cleanupRef.current = true;
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  useEffect(() => {
    if (cleanupRef.current) {
      setQuery('');
      setActiveIndex(-1);
      setSelectedCategory('All');
      cleanupRef.current = false;
    }
  }, [isOpen, setQuery]);

  useEffect(() => {
    const handleClickOutside = (e) => {
      if (modalRef.current && !modalRef.current.contains(e.target)) {
        onClose();
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen, onClose]);

  const handleInputChange = useCallback((e) => {
    const value = e.target.value;
    setQuery(value);
    setActiveIndex(-1);
    if (value.trim()) {
      search(value, selectedCategory, filters);
    }
  }, [setQuery, search, selectedCategory, filters]);

  const handleSelect = useCallback((result) => {
    addRecentSearch(result.title);
    if (result.type === 'game') {
      navigate(`/game/${result.id}`);
    } else {
      navigate(`/profile/${result.id}`);
    }
    onClose();
  }, [addRecentSearch, navigate, onClose]);

  const handleRecentSelect = useCallback((recent) => {
    setQuery(recent);
    search(recent, selectedCategory, filters);
  }, [setQuery, search, selectedCategory, filters]);

  const getTotalItems = useCallback(() => {
    if (selectedCategory === 'All') {
      return groupedResults.games.length + groupedResults.users.length;
    }
    if (selectedCategory === 'Games') {
      return groupedResults.games.length;
    }
    if (selectedCategory === 'Users') {
      return groupedResults.users.length;
    }
    return 0;
  }, [groupedResults, selectedCategory]);

  const handleKeyDown = useCallback((e) => {
    const totalItems = getTotalItems();

    if (!totalItems && !recentSearches.length) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setActiveIndex(prev => (prev < totalItems - 1 ? prev + 1 : 0));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setActiveIndex(prev => (prev > 0 ? prev - 1 : totalItems - 1));
        break;
      case 'Enter':
        e.preventDefault();
        if (activeIndex >= 0 && flatResults[activeIndex]) {
          handleSelect(flatResults[activeIndex]);
        } else if (query.trim() && totalItems === 0) {
          addRecentSearch(query);
          onClose();
        }
        break;
    }
  }, [activeIndex, flatResults, query, recentSearches, handleSelect, addRecentSearch, onClose, getTotalItems]);

  const handleCategoryChange = useCallback((cat) => {
    setSelectedCategory(cat);
    if (query.trim()) {
      search(query, cat, filters);
    }
  }, [query, search, filters]);

  const handleFilterChange = useCallback((key, value) => {
    const newFilters = { ...filters };
    if (value === '' || value == null) {
      delete newFilters[key];
    } else {
      newFilters[key] = value;
    }
    setFilters(newFilters);
    if (query.trim()) {
      search(query, selectedCategory, newFilters);
    }
  }, [filters, setFilters, query, search, selectedCategory]);

  const renderResultsGroup = (title, items, icon, type) => {
    if (!items.length) return null;

    return (
      <div className="search-results-group">
        <div className="search-results-group-header">
          {icon}
          {title}
        </div>
        <ul className="search-results-list">
          {items.map((item, index) => {
            const flatIndex = type === 'games' ? index : groupedResults.games.length + index;
            return (
              <li
                key={`${type}-${item.id}`}
                className={`search-result-item ${flatIndex === activeIndex ? 'active' : ''}`}
                onClick={() => handleSelect(item)}
                onMouseEnter={() => setActiveIndex(flatIndex)}
                role="option"
                aria-selected={flatIndex === activeIndex}
              >
                <span className="result-icon">
                  {type === 'games' ? (
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <rect x="2" y="6" width="20" height="12" rx="2" />
                      <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01" />
                    </svg>
                  ) : (
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                      <circle cx="12" cy="7" r="4" />
                    </svg>
                  )}
                </span>
                <span className="result-content">
                  <span className="result-title">{item.title}</span>
                  <span className="result-subtitle">{item.subtitle}</span>
                </span>
                <span className={`result-badge ${type === 'games' ? 'game' : 'user'}`}>
                  {type === 'games' ? 'Game' : 'User'}
                </span>
              </li>
            );
          })}
        </ul>
      </div>
    );
  };

  if (!isOpen) return null;

  return (
    <div className="search-modal-overlay" role="dialog" aria-modal="true" aria-label="Search">
      <div className="search-modal" ref={modalRef}>
        <div className="search-modal-header">
          <svg className="search-modal-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" />
          </svg>
          <input
            id="search-modal-input"
            ref={inputRef}
            type="text"
            value={query}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            placeholder="Search games, users..."
            className="search-modal-input"
            autoComplete="off"
          />
          {loading && <Spinner size="sm" />}
          <kbd className="search-modal-kbd">ESC</kbd>
        </div>

        <div className="search-modal-filters">
          {CATEGORY_FILTERS.map(cat => (
            <button
              key={cat}
              type="button"
              className={`search-filter-btn ${selectedCategory === cat ? 'active' : ''}`}
              onClick={() => handleCategoryChange(cat)}
            >
              {cat === 'All' && (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <circle cx="12" cy="12" r="10" />
                  <path d="M12 6v6l4 2" />
                </svg>
              )}
              {cat === 'Games' && (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="2" y="6" width="20" height="12" rx="2" />
                  <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01" />
                </svg>
              )}
              {cat === 'Users' && (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                  <circle cx="12" cy="7" r="4" />
                </svg>
              )}
              {cat}
            </button>
          ))}
          {(selectedCategory === 'All' || selectedCategory === 'Games') && (
            <button
              type="button"
              className={`search-filter-btn ${showFilters || Object.keys(filters).length > 0 ? 'active' : ''}`}
              onClick={() => setShowFilters(v => !v)}
              aria-label="Toggle genre/tag filters"
              title="Filters"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
              </svg>
              Filters{Object.keys(filters).length > 0 ? ` (${Object.keys(filters).length})` : ''}
            </button>
          )}
        </div>

        {showFilters && (selectedCategory === 'All' || selectedCategory === 'Games') && (
          <div className="search-modal-advanced-filters" style={{ padding: '0.5rem 1rem', borderBottom: '1px solid var(--color-border)', display: 'flex', gap: '0.75rem', flexWrap: 'wrap', alignItems: 'center', background: 'var(--color-bg-secondary)' }}>
            <select
              value={filters.genre ?? ''}
              onChange={e => handleFilterChange('genre', e.target.value || null)}
              style={{ fontSize: '0.75rem', padding: '0.25rem 0.5rem', background: 'var(--color-bg-card)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)', color: 'var(--color-text-primary)', cursor: 'pointer' }}
              aria-label="Filter by genre"
            >
              <option value="">All Genres</option>
              {(genres ?? []).map(g => (
                <option key={g} value={g.toLowerCase()}>{g}</option>
              ))}
            </select>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', fontSize: '0.75rem', color: 'var(--color-text-secondary)', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={filters.is_free === 'true'}
                onChange={e => handleFilterChange('is_free', e.target.checked ? 'true' : null)}
                aria-label="Free to play only"
              />
              Free to play
            </label>
            {Object.keys(filters).length > 0 && (
              <button
                type="button"
                style={{ fontSize: '0.7rem', color: 'var(--color-text-muted)', background: 'none', border: 'none', cursor: 'pointer', textDecoration: 'underline', padding: 0 }}
                onClick={() => {
                  setFilters({});
                  if (query.trim()) search(query, selectedCategory, {});
                }}
              >
                Clear filters
              </button>
            )}
          </div>
        )}

        <div className="search-modal-content">
          {!query.trim() && recentSearches.length > 0 && (
            <div className="search-recent">
              <div className="search-recent-header">
                <span>Recent Searches</span>
                <button type="button" onClick={clearRecentSearches} className="search-clear-recent">
                  Clear all
                </button>
              </div>
              <ul className="search-recent-list">
                {recentSearches.map((recent, i) => (
                  <li key={i}>
                    <button
                      type="button"
                      className="search-recent-item"
                      onClick={() => handleRecentSelect(recent)}
                    >
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10" />
                        <polyline points="12 6 12 12 16 14" />
                      </svg>
                      {recent}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {query.trim() && results && (
            <>
              {(selectedCategory === 'All' || selectedCategory === 'Games') &&
                renderResultsGroup(
                  'Games',
                  groupedResults.games,
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="2" y="6" width="20" height="12" rx="2" />
                    <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01" />
                  </svg>,
                  'games'
                )}
              {(selectedCategory === 'All' || selectedCategory === 'Users') &&
                renderResultsGroup(
                  'Users',
                  groupedResults.users,
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>,
                  'users'
                )}
            </>
          )}

          {query.trim() && !loading && results && flatResults.length === 0 && (
            <div className="search-empty">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <circle cx="11" cy="11" r="8" />
                <path d="M21 21l-4.35-4.35" />
              </svg>
              <p>No results found for "{query}"</p>
              <span>Try searching for something else</span>
            </div>
          )}

          {query.trim() && loading && (
            <div className="search-loading">
              <Spinner size="md" />
            </div>
          )}
        </div>

        <div className="search-modal-footer">
          <div className="search-shortcuts">
            <span><kbd>↑</kbd><kbd>↓</kbd> Navigate</span>
            <span><kbd>↵</kbd> Select</span>
            <span><kbd>esc</kbd> Close</span>
          </div>
          <div className="search-modal-brand">
            <span className="logo-icon">M</span>
            <span>Magnetite</span>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SearchModal;
