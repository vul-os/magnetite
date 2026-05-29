import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function Error({ errorId, message, onRetry }) {
  const displayMessage = message || 'An unexpected error occurred. Our team has been notified.';

  return (
    <Layout>
      <div className="error-page">
        <div className="error-illustration">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
        </div>
        <div className="error-code">Oops!</div>
        <h1 className="error-title">Something went wrong</h1>
        <p className="error-message">{displayMessage}</p>
        {errorId && (
          <div className="error-id">
            <span className="error-id-label">Error ID:</span>
            <code>{errorId}</code>
          </div>
        )}
        <div className="error-actions">
          {onRetry && (
            <Button onClick={onRetry} variant="primary" size="lg">
              Try Again
            </Button>
          )}
          <Link to="/" className="btn btn-secondary btn-lg">
            Go Home
          </Link>
        </div>
      </div>
    </Layout>
  );
}
