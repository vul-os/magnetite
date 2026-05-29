import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function ServerError({ onRetry }) {
  return (
    <Layout>
      <div className="error-page reveal">
        <div className="error-illustration reveal-1">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
            <rect x="2" y="3" width="20" height="14" rx="2" />
            <path d="M8 21h8M12 17v4" />
            <path d="M12 7v4M12 13v.5" />
          </svg>
        </div>

        <div className="error-code reveal-2" data-code="500" aria-label="Error 500">500</div>

        <h1 className="error-title reveal-3">Something went wrong</h1>

        <p className="error-message reveal-4">
          The server panicked. Our team has been notified and is patching the deployment. Should be back online shortly.
        </p>

        <div className="error-actions reveal-5">
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
