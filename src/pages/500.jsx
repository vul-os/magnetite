import Layout from '../components/Layout';
import Button from '../components/common/Button';
import './error-pages.css';

export default function ServerErrorPage({ onRetry }) {
  return (
    <Layout>
      <div className="error-page">
        <div className="error-illustration error-illustration--server">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <rect x="2" y="3" width="20" height="6" rx="1" />
            <rect x="2" y="15" width="20" height="6" rx="1" />
            <circle cx="6" cy="6" r="1" fill="currentColor" />
            <circle cx="6" cy="18" r="1" fill="currentColor" />
            <path d="M8 6h8M8 18h8" strokeDasharray="2 2" />
          </svg>
        </div>
        <div className="error-code">500</div>
        <h1 className="error-title">Something went wrong</h1>
        <p className="error-message">
          We're experiencing some technical difficulties. Please try again in a few moments.
        </p>
        <div className="error-actions">
          {onRetry && (
            <Button onClick={onRetry} variant="primary" size="lg">
              Try Again
            </Button>
          )}
          <a href="mailto:support@magnetite.gg" className="btn btn-secondary btn-lg">
            Contact Support
          </a>
        </div>
      </div>
    </Layout>
  );
}
