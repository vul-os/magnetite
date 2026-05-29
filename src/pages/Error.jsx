import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function Error({ errorId, message, onRetry }) {
  const displayMessage = message || 'An unexpected error occurred. Our team has been notified.';

  return (
    <Layout>
      <div className="error-page reveal">
        <div className="error-illustration reveal-1">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </div>

        <div className="error-code reveal-2" data-code="Oops" aria-label="Error">Oops!</div>

        <h1 className="error-title reveal-3">Something went wrong</h1>

        <p className="error-message reveal-4">{displayMessage}</p>

        {errorId && (
          <div className="error-id reveal-5">
            <span className="error-id-label">Error ID</span>
            <code>{errorId}</code>
          </div>
        )}

        <div className="error-actions reveal-5">
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
