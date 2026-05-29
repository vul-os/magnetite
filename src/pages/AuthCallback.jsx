import { useEffect, useState, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

const TOKEN_KEY = 'magnetite_token';
const USER_KEY  = 'magnetite_user';

/* Defined at module level to satisfy react-hooks/static-components */
function AuthShell({ children }) {
  return (
    <div className="auth-page">
      <div className="auth-background" aria-hidden="true">
        <div className="auth-bg-gradient" />
        <div className="auth-bg-glow auth-bg-glow-1" />
        <div className="auth-bg-glow auth-bg-glow-2" />
      </div>
      <div className="auth-container-center">
        <div className="auth-card">
          <div className="auth-logo-container" style={{ justifyContent: 'center', marginBottom: '0.5rem' }}>
            <div className="auth-logo-icon">M</div>
            <span className="auth-logo-text">Magnetite</span>
          </div>
          {children}
        </div>
      </div>
    </div>
  );
}

export default function AuthCallback() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const processedRef = useRef(false);

  const errorParam   = searchParams.get('error');
  const initialError = errorParam ? decodeURIComponent(errorParam) : '';

  const [callbackState, setCallbackState] = useState(
    initialError
      ? { type: 'error', error: initialError, success: '' }
      : { type: 'processing', error: '', success: '' }
  );

  useEffect(() => {
    if (callbackState.type === 'error' || processedRef.current) return;
    processedRef.current = true;

    const action      = searchParams.get('action');
    const destination = searchParams.get('destination') || '/';

    let token = searchParams.get('token');
    if (!token) {
      const hash = window.location.hash;
      if (hash && hash.startsWith('#token=')) token = hash.slice(7);
    }

    const processCallback = async () => {
      if (!token) {
        setCallbackState({ type: 'error', error: 'No authentication token received', success: '' });
        return;
      }

      try {
        if (action === 'link') {
          await api.auth.linkAccount(token);
          setCallbackState({ type: 'success', error: '', success: 'Account linked successfully' });
          setTimeout(() => navigate('/settings/connected-accounts', { replace: true }), 1500);
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
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--processing" aria-label="Signing in">
            <span className="spinner spinner-md" style={{ color: 'var(--color-accent)' }} />
          </div>
          <div>
            <h1 className="auth-state-title">Signing in</h1>
            <p className="auth-state-body">
              Completing authentication, please wait&hellip;
            </p>
          </div>
        </div>
      </AuthShell>
    );
  }

  if (type === 'error') {
    return (
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">✕</div>
          <div>
            <h1 className="auth-state-title">Authentication failed</h1>
            <p className="auth-state-body">{error}</p>
          </div>
          <button
            className="auth-submit-btn"
            onClick={() => navigate('/login', { replace: true })}
            style={{ width: '100%' }}
          >
            Return to Sign In
          </button>
        </div>
      </AuthShell>
    );
  }

  if (type === 'success') {
    return (
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
          <div>
            <h1 className="auth-state-title">Account linked!</h1>
            <p className="auth-state-body">
              {success} Redirecting to connected accounts&hellip;
            </p>
          </div>
        </div>
      </AuthShell>
    );
  }

  return null;
}
