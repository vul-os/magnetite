import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import Button from '../components/common/Button';
import '../styles/error-pages.css';

export default function Forbidden({ reason }) {
  const navigate = useNavigate();
  const displayReason = reason || 'You do not have permission to access this resource. This could be because:';

  const reasons = [
    'You are not logged in',
    'Your account has been suspended',
    'This content is not available for your account type',
    'The resource requires higher privileges'
  ];

  return (
    <Layout>
      <div className="error-page">
        <div className="error-illustration">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
            <path d="M7 11V7a5 5 0 0 1 10 0v4" />
          </svg>
        </div>
        <div className="error-code">403</div>
        <h1 className="error-title">Access denied</h1>
        <p className="error-message">{displayReason}</p>
        <ul className="error-reasons">
          {reasons.map((r, i) => (
            <li key={i}>{r}</li>
          ))}
        </ul>
        <div className="error-actions">
          <Button onClick={() => navigate(-1)} variant="secondary" size="lg">
            Go Back
          </Button>
          <Link to="/" className="btn btn-primary btn-lg">
            Go Home
          </Link>
        </div>
      </div>
    </Layout>
  );
}
