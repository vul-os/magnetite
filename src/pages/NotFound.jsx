import { useState } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function NotFound() {
  const [searchQuery, setSearchQuery] = useState('');

  const handleSearch = (e) => {
    e.preventDefault();
    if (searchQuery.trim()) {
      window.location.href = `/?search=${encodeURIComponent(searchQuery)}`;
    }
  };

  return (
    <Layout>
      <div className="error-page reveal">
        <div className="error-illustration reveal-1">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </div>

        <div className="error-code reveal-2" data-code="404" aria-label="Error 404">404</div>

        <h1 className="error-title reveal-3">Page not found</h1>

        <p className="error-message reveal-4">
          Looks like this page compiled to nothing. It might have been deleted, moved, or never existed.
        </p>

        <form className="error-search reveal-5" onSubmit={handleSearch}>
          <input
            type="text"
            placeholder="Search for Rust games..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="search-input"
            aria-label="Search games"
          />
          <Button type="submit" variant="secondary">Search</Button>
        </form>

        <div className="error-actions reveal-6">
          <Link to="/" className="btn btn-primary btn-lg">
            Go Home
          </Link>
          <Link to="/marketplace" className="btn btn-secondary btn-lg">
            Browse Games
          </Link>
        </div>
      </div>
    </Layout>
  );
}
