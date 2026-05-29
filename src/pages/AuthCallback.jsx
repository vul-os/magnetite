import { useEffect, useState, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

const TOKEN_KEY = 'magnetite_token';
const USER_KEY = 'magnetite_user';

export default function AuthCallback() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const processedRef = useRef(false);

  // Derive initial error state from URL params synchronously (no effect needed)
  const errorParam = searchParams.get('error');
  const initialError = errorParam ? decodeURIComponent(errorParam) : '';

  const [callbackState, setCallbackState] = useState(
    initialError
      ? { type: 'error', error: initialError, success: '' }
      : { type: 'processing', error: '', success: '' }
  );

  useEffect(() => {
    // Don't run again if we already have an error from URL params or already processed
    if (callbackState.type === 'error' || processedRef.current) return;
    processedRef.current = true;

    const action = searchParams.get('action');
    const destination = searchParams.get('destination') || '/';

    let token = searchParams.get('token');
    if (!token) {
      const hash = window.location.hash;
      if (hash && hash.startsWith('#token=')) {
        token = hash.slice(7);
      }
    }

    if (!token) {
      setCallbackState({ type: 'error', error: 'No authentication token received', success: '' });
      return;
    }

    const processCallback = async () => {
      try {
        if (action === 'link') {
          await api.auth.linkAccount(token);
          setCallbackState({ type: 'success', error: '', success: 'Account linked successfully' });
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
        setCallbackState({ type: 'error', error: err.message || 'Authentication failed', success: '' });
      }
    };

    processCallback();
  }, [searchParams, navigate, callbackState.type]);

  const { type, error, success } = callbackState;

  if (type === 'processing') {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card" style={{ textAlign: 'center' }}>
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
                <span className="auth-logo-text">Magnetite</span>
              </div>
              <h1 className="auth-title">Signing In</h1>
              <p className="auth-subtitle">Completing authentication, please wait...</p>
            </div>
            <div className="auth-body" style={{ alignItems: 'center' }}>
              <span className="spinner" style={{ width: 32, height: 32, color: 'var(--color-accent)', borderWidth: 3 }} />
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (type === 'error') {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
                <span className="auth-logo-text">Magnetite</span>
              </div>
              <h1 className="auth-title">Authentication Failed</h1>
            </div>
            <div className="auth-body">
              <div className="auth-error">
                <span className="auth-error-icon">!</span>
                {error}
              </div>
              <button
                className="auth-submit-btn"
                onClick={() => navigate('/login', { replace: true })}
              >
                Return to Login
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (type === 'success') {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card" style={{ textAlign: 'center' }}>
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
                <span className="auth-logo-text">Magnetite</span>
              </div>
              <h1 className="auth-title">Account Linked!</h1>
              <p className="auth-subtitle">{success}</p>
            </div>
            <div className="auth-body">
              <p style={{ color: 'var(--color-text-muted)', fontSize: 13, textAlign: 'center', margin: 0 }}>
                Redirecting to connected accounts...
              </p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return null;
}
