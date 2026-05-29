import { useState, useEffect } from 'react';
import { useSearchParams, Link } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

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

  const renderContent = () => {
    if (status === 'loading') {
      return (
        <>
          <div className="auth-header">
            <div className="auth-logo-container">
              <div className="auth-logo-icon">M</div>
              <span className="auth-logo-text">Magnetite</span>
            </div>
            <h1 className="auth-title">Verifying Email</h1>
            <p className="auth-subtitle">Please wait while we verify your email address...</p>
          </div>
          <div className="auth-body" style={{ alignItems: 'center' }}>
            <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)', width: 32, height: 32 }} />
          </div>
        </>
      );
    }

    if (status === 'invalid') {
      return (
        <>
          <div className="auth-header">
            <div className="auth-logo-container">
              <div className="auth-logo-icon">M</div>
              <span className="auth-logo-text">Magnetite</span>
            </div>
            <h1 className="auth-title">Invalid Link</h1>
            <p className="auth-subtitle">This email verification link is invalid or has expired.</p>
          </div>
          <div className="auth-body">
            <Link
              to="/register"
              className="auth-submit-btn"
              style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            >
              Sign Up
            </Link>
          </div>
        </>
      );
    }

    if (status === 'success') {
      return (
        <>
          <div className="auth-header">
            <div className="auth-logo-container">
              <div className="auth-logo-icon">M</div>
              <span className="auth-logo-text">Magnetite</span>
            </div>
            <h1 className="auth-title">Email Verified!</h1>
            <p className="auth-subtitle">Your email has been successfully verified.</p>
          </div>
          <div className="auth-body">
            <Link
              to="/login"
              className="auth-submit-btn"
              style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            >
              Log In
            </Link>
          </div>
        </>
      );
    }

    return (
      <>
        <div className="auth-header">
          <div className="auth-logo-container">
            <div className="auth-logo-icon">M</div>
            <span className="auth-logo-text">Magnetite</span>
          </div>
          <h1 className="auth-title">Verification Failed</h1>
          <p className="auth-subtitle">{error || 'This verification link has expired.'}</p>
        </div>
        <div className="auth-body">
          <button
            className="auth-submit-btn"
            onClick={handleResend}
            disabled={resending || resent}
          >
            {resending ? <span className="spinner spinner-sm" /> : resent ? 'Email Resent!' : 'Resend Verification Email'}
          </button>
          {resent && (
            <p style={{ color: 'var(--color-text-muted)', fontSize: 13, textAlign: 'center', margin: 0 }}>
              Check your inbox for a new verification link.
            </p>
          )}
          <div className="auth-footer" style={{ textAlign: 'center' }}>
            Already verified? <Link to="/login" className="auth-link-forgot">Log in</Link>
          </div>
        </div>
      </>
    );
  };

  return (
    <div className="auth-page">
      <div className="auth-background">
        <div className="auth-bg-gradient" />
        <div className="auth-bg-glow auth-bg-glow-1" />
        <div className="auth-bg-glow auth-bg-glow-2" />
      </div>
      <div className="auth-container-center">
        <div className="auth-card">
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
