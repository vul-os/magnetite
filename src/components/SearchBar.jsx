import { useState, useRef, useEffect, useCallback } from 'react';
import { useSearch } from '../hooks/useSearch';
import Spinner from './common/Spinner';

export default function SearchBar({
  onSearch,
  placeholder = 'Search games, users...',
  autoFocus = false,
  onResultSelect,
}) {
  const { search, results, isLoading, categories } = useSearch();
  const [query, setQuery] = useState('');
  const [isOpen, setIsOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [selectedCategory, setSelectedCategory] = useState('All');
  const inputRef = useRef(null);
  const containerRef = useRef(null);

  const allResults = results
    ? [...(results.games || []).map(g => ({ ...g, category: 'game' })), ...(results.users || []).map(u => ({ ...u, category: 'user' }))]
    : [];

  useEffect(() => {
    if (autoFocus && inputRef.current) {
      inputRef.current.focus();
    }
  }, [autoFocus]);

  useEffect(() => {
    const handleClickOutside = (e) => {
      if (containerRef.current && !containerRef.current.contains(e.target)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleInputChange = useCallback((e) => {
    const value = e.target.value;
    setQuery(value);
    setActiveIndex(-1);
    search(value, selectedCategory).then((data) => {
      if (data && (data.games?.length || data.users?.length)) {
        setIsOpen(true);
      }
    });
  }, [search, selectedCategory]);

  const handleClear = useCallback(() => {
    setQuery('');
    setIsOpen(false);
    setActiveIndex(-1);
    inputRef.current?.focus();
  }, []);

  const handleSelect = useCallback((result) => {
    setQuery(result.title);
    setIsOpen(false);
    setActiveIndex(-1);
    onResultSelect?.(result);
    onSearch?.(result.title, result.category);
  }, [onResultSelect, onSearch]);

  const handleKeyDown = useCallback((e) => {
    if (!isOpen || allResults.length === 0) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setActiveIndex(prev => (prev < allResults.length - 1 ? prev + 1 : 0));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setActiveIndex(prev => (prev > 0 ? prev - 1 : allResults.length - 1));
        break;
      case 'Enter':
        e.preventDefault();
        if (activeIndex >= 0 && allResults[activeIndex]) {
          handleSelect(allResults[activeIndex]);
        } else if (query.trim()) {
          onSearch?.(query);
        }
        break;
      case 'Escape':
        setIsOpen(false);
        setActiveIndex(-1);
        break;
    }
  }, [isOpen, allResults, activeIndex, query, onSearch, handleSelect]);

  const handleCategoryChange = useCallback((cat) => {
    setSelectedCategory(cat);
    if (query.trim()) {
      search(query, cat);
    }
  }, [query, search]);

  return (
    <div className="search-bar-container" ref={containerRef}>
      <div className="search-bar-input-wrapper">
        <svg className="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="11" cy="11" r="8" />
          <path d="M21 21l-4.35-4.35" />
        </svg>
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          onFocus={() => query.trim() && results && setIsOpen(true)}
          placeholder={placeholder}
          className="search-bar-input"
          role="combobox"
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          aria-autocomplete="list"
        />
        {isLoading && <Spinner size="sm" className="search-spinner" />}
        {query && !isLoading && (
          <button
            type="button"
            onClick={handleClear}
            className="search-clear-btn"
            aria-label="Clear search"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        )}
      </div>

      {isOpen && (
        <div className="search-dropdown" role="listbox">
          <div className="search-categories">
            {categories.map(cat => (
              <button
                key={cat}
                type="button"
                className={`search-category-btn ${selectedCategory === cat ? 'active' : ''}`}
                onClick={() => handleCategoryChange(cat)}
              >
                {cat}
              </button>
            ))}
          </div>

          {allResults.length > 0 ? (
            <ul className="search-results-list">
              {allResults.map((result, index) => (
                <li
                  key={`${result.type}-${result.id}`}
                  className={`search-result-item ${index === activeIndex ? 'active' : ''}`}
                  onClick={() => handleSelect(result)}
                  onMouseEnter={() => setActiveIndex(index)}
                  role="option"
                  aria-selected={index === activeIndex}
                >
                  <span className="result-icon">
                    {result.type === 'game' ? (
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="2" y="6" width="20" height="12" rx="2" />
                        <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01" />
                      </svg>
                    ) : (
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                        <circle cx="12" cy="7" r="4" />
                      </svg>
                    )}
                  </span>
                  <span className="result-content">
                    <span className="result-title">{result.title}</span>
                    <span className="result-subtitle">{result.subtitle}</span>
                  </span>
                  <span className="result-type">{result.type === 'game' ? 'Game' : 'User'}</span>
                </li>
              ))}
            </ul>
          ) : (
            <div className="search-empty">
              <p>No results found for "{query}"</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
