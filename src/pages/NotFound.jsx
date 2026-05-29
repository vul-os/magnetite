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
      <div className="error-page">
        <div className="error-illustration">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </div>
        <div className="error-code">404</div>
        <h1 className="error-title">Page not found</h1>
        <p className="error-message">
          Looks like this page took a wrong turn somewhere. It might have been deleted, moved, or never existed.
        </p>
        <form className="error-search" onSubmit={handleSearch}>
          <input
            type="text"
            placeholder="Search for games..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="search-input"
          />
          <Button type="submit" variant="secondary">Search</Button>
        </form>
        <div className="error-actions">
          <Link to="/" className="btn btn-primary btn-lg">
            Go Home
          </Link>
        </div>
      </div>
    </Layout>
  );
}
