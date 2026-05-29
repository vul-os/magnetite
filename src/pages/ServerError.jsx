import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function ServerError({ onRetry }) {
  return (
    <Layout>
      <div className="error-page">
        <div className="error-illustration">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 6v6l4 2" />
          </svg>
        </div>
        <div className="error-code">500</div>
        <h1 className="error-title">Something went wrong</h1>
        <p className="error-message">
          We're experiencing some technical difficulties. Our team has been notified and is working on it.
        </p>
        <div className="error-actions">
          {onRetry && (
            <Button onClick={onRetry} variant="primary" size="lg">
              Try Again
            </Button>
          )}
          <Link to="/" className="btn btn-secondary btn-lg">
            Go Home
          </Link>
          <a href="mailto:support@magnetite.gg" className="btn btn-ghost btn-lg">
            Contact Support
          </a>
        </div>
      </div>
    </Layout>
  );
}
