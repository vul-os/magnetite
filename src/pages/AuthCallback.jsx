import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { api } from '../api/client';

const TOKEN_KEY = 'magnetite_token';
const USER_KEY = 'magnetite_user';

export default function AuthCallback() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [isProcessing, setIsProcessing] = useState(true);

  useEffect(() => {
    const action = searchParams.get('action');
    const errorParam = searchParams.get('error');
    const destination = searchParams.get('destination') || '/';

    if (errorParam) {
      setError(decodeURIComponent(errorParam));
      setIsProcessing(false);
      return;
    }

    let token = searchParams.get('token');

    if (!token) {
      const hash = window.location.hash;
      if (hash && hash.startsWith('#token=')) {
        token = hash.slice(7);
      }
    }

    if (!token) {
      setError('No authentication token received');
      setIsProcessing(false);
      return;
    }

    const processCallback = async () => {
      try {
        if (action === 'link') {
          await api.auth.linkAccount(token);
          setSuccess('Account linked successfully');
          setIsProcessing(false);
          setTimeout(() => {
            navigate('/settings/connected-accounts', { replace: true });
          }, 1500);
        } else {
          localStorage.setItem(TOKEN_KEY, token);

          const user = await api.auth.me();
          localStorage.setItem(USER_KEY, JSON.stringify(user));

          navigate(destination, { replace: true });
        }
      } catch (err) {
        localStorage.removeItem(TOKEN_KEY);
        setError(err.message || 'Authentication failed');
        setIsProcessing(false);
      }
    };

    processCallback();
  }, [searchParams, navigate]);

  if (isProcessing) {
    return (
      <div className="auth-callback-container">
        <div className="callback-content">
          <div className="spinner large"></div>
          <p>Completing sign in...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="auth-callback-container">
        <div className="callback-content">
          <div className="error-icon">!</div>
          <h2>Authentication Failed</h2>
          <p className="error-message">{error}</p>
          <div className="callback-actions">
            <button onClick={() => navigate('/login', { replace: true })}>
              Return to Login
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (success) {
    return (
      <div className="auth-callback-container">
        <div className="callback-content">
          <div className="success-icon">&#10003;</div>
          <h2>Account Linked</h2>
          <p className="success-message">{success}</p>
          <p>Redirecting to connected accounts...</p>
        </div>
      </div>
    );
  }

  return null;
}
