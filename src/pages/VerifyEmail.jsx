import { useState, useEffect } from 'react';
import { useSearchParams, Link } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

/* Shared auth page shell — defined at module level to avoid lint errors */
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

export default function VerifyEmail() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token');

  const [status, setStatus] = useState(token ? 'loading' : 'invalid');
  const [error, setError] = useState('');
  const [resending, setResending] = useState(false);
  const [resent, setResent] = useState(false);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;

    async function verify() {
      try {
        await api.auth.verifyEmail(token);
        if (!cancelled) setStatus('success');
      } catch (err) {
        if (!cancelled) {
          setError(err.message || 'Verification failed');
          setStatus('error');
        }
      }
    }
    verify();
    return () => { cancelled = true; };
  }, [token]);

  const handleResend = async () => {
    setResending(true);
    setError('');
    try {
      await api.auth.resendVerification(token);
      setResent(true);
    } catch (err) {
      setError(err.message || 'Failed to resend verification email');
    } finally {
      setResending(false);
    }
  };

  if (status === 'loading') {
    return (
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--processing" aria-label="Verifying">
            <span className="spinner spinner-md" style={{ color: 'var(--color-accent)' }} />
          </div>
          <div>
            <h1 className="auth-state-title">Verifying email</h1>
            <p className="auth-state-body">
              Please wait while we verify your email address&hellip;
            </p>
          </div>
        </div>
      </AuthShell>
    );
  }

  if (status === 'invalid') {
    return (
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">✕</div>
          <div>
            <h1 className="auth-state-title">Invalid link</h1>
            <p className="auth-state-body">
              This email verification link is invalid or has expired.
              Create a new account or sign in to request a new link.
            </p>
          </div>
          <Link to="/register" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
            Sign Up
          </Link>
        </div>
      </AuthShell>
    );
  }

  if (status === 'success') {
    return (
      <AuthShell>
        <div className="auth-state-card">
          <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
          <div>
            <h1 className="auth-state-title">Email verified!</h1>
            <p className="auth-state-body">
              Your email address has been successfully verified. You can now sign in
              to your Magnetite account.
            </p>
          </div>
          <Link to="/login" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
            Sign In
          </Link>
        </div>
      </AuthShell>
    );
  }

  /* error state */
  return (
    <AuthShell>
      <div className="auth-state-card">
        <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">!</div>
        <div>
          <h1 className="auth-state-title">Verification failed</h1>
          <p className="auth-state-body">
            {error || 'This verification link has expired.'}
          </p>
        </div>

        <button
          className="auth-submit-btn"
          onClick={handleResend}
          disabled={resending || resent}
          style={{ width: '100%' }}
        >
          {resending
            ? <span className="spinner spinner-sm" aria-hidden="true" />
            : resent
              ? 'Email Resent!'
              : 'Resend Verification Email'}
        </button>

        {resent && (
          <div className="auth-success" role="status">
            <span className="auth-success-icon" aria-hidden="true">✓</span>
            Check your inbox for a new verification link.
          </div>
        )}

        {error && !resent && (
          <div className="auth-error" role="alert">
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {error}
          </div>
        )}

        <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: 0 }}>
          Already verified?{' '}
          <Link to="/login" className="auth-link-forgot">Sign in</Link>
        </p>
      </div>
    </AuthShell>
  );
}
