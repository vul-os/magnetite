import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import './error-pages.css';

export default function Forbidden() {
  const navigate = useNavigate();

  return (
    <Layout>
      <div className="error-page">
        <div className="error-illustration error-illustration--lock">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
            <path d="M7 11V7a5 5 0 0 1 10 0v4" />
            <circle cx="12" cy="16" r="1" fill="currentColor" />
          </svg>
        </div>
        <div className="error-code">403</div>
        <h1 className="error-title">Access Denied</h1>
        <p className="error-message">
          You don't have permission to access this page.
        </p>
        <div className="error-actions">
          <Button onClick={() => navigate('/')} variant="primary" size="lg">
            Go Back Home
          </Button>
        </div>
      </div>
    </Layout>
  );
}
