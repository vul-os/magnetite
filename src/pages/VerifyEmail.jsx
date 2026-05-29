import { useState, useEffect } from 'react';
import { useSearchParams, Link } from 'react-router-dom';
import { api } from '../api/client';

export default function VerifyEmail() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token');

  const [status, setStatus] = useState('loading');
  const [error, setError] = useState('');
  const [resending, setResending] = useState(false);
  const [resent, setResent] = useState(false);

  useEffect(() => {
    if (!token) {
      setStatus('invalid');
      return;
    }

    async function verify() {
      try {
        await api.auth.verifyEmail(token);
        setStatus('success');
      } catch (err) {
        setError(err.message || 'Verification failed');
        setStatus('error');
      }
    }
    verify();
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
      <div className="auth-container">
        <div className="loading-state">
          <span className="spinner large" />
          <p>Verifying your email...</p>
        </div>
      </div>
    );
  }

  if (status === 'invalid') {
    return (
      <div className="auth-container">
        <div className="error-state">
          <h2>Invalid Verification Link</h2>
          <p>This email verification link is invalid or has expired.</p>
          <Link to="/register" className="btn btn-primary">Sign Up</Link>
        </div>
      </div>
    );
  }

  if (status === 'success') {
    return (
      <div className="auth-container">
        <div className="success-message">
          <h2>Email Verified!</h2>
          <p>Your email has been successfully verified.</p>
          <Link to="/login" className="btn btn-primary">Log In</Link>
        </div>
      </div>
    );
  }

  return (
    <div className="auth-container">
      <div className="error-state">
        <h2>Verification Failed</h2>
        <p>{error || 'This verification link has expired.'}</p>
        <button
          onClick={handleResend}
          disabled={resending || resent}
          className="btn btn-primary"
        >
          {resending ? <span className="spinner" /> : resent ? 'Email Resent!' : 'Resend Verification Email'}
        </button>
        {resent && (
          <p className="muted" style={{ marginTop: '12px' }}>Check your inbox for a new verification link.</p>
        )}
      </div>
      <p className="auth-footer">
        Already verified? <Link to="/login">Log in</Link>
      </p>
    </div>
  );
}