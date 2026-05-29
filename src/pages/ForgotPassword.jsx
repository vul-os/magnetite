import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

export default function ForgotPassword() {
  const [email, setEmail] = useState('');
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await api.auth.forgotPassword(email);
      setSuccess(true);
    } catch (err) {
      setError(err.message || 'Failed to send reset link');
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
          <div className="auth-bg-glow auth-bg-glow-2" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
                <span className="auth-logo-text">Magnetite</span>
              </div>
              <h1 className="auth-title">Check Your Email</h1>
              <p className="auth-subtitle">
                We sent a reset link to <strong style={{ color: 'var(--color-accent)' }}>{email}</strong>
              </p>
            </div>
            <div className="auth-body">
              <p style={{ color: 'var(--color-text-secondary)', fontSize: 14, textAlign: 'center', margin: 0 }}>
                Check your inbox and follow the instructions to reset your password. The link expires in 1 hour.
              </p>
              <Link
                to="/login"
                className="auth-submit-btn"
                style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
              >
                Back to Login
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="auth-page">
      <div className="auth-background">
        <div className="auth-bg-gradient" />
        <div className="auth-bg-glow auth-bg-glow-1" />
        <div className="auth-bg-glow auth-bg-glow-2" />
        <div className="auth-bg-particles" />
      </div>

      <div className="auth-container-center">
        <div className="auth-card">
          <div className="auth-header">
            <div className="auth-logo-container">
              <div className="auth-logo-icon">M</div>
              <span className="auth-logo-text">Magnetite</span>
            </div>
            <h1 className="auth-title">Forgot Password</h1>
            <p className="auth-subtitle">Enter your email and we&apos;ll send you a reset link</p>
          </div>

          <div className="auth-body">
            {error && (
              <div className="auth-error">
                <span className="auth-error-icon">!</span>
                {error}
              </div>
            )}

            <form onSubmit={handleSubmit} className="auth-form">
              <div className="email-input-wrapper">
                <div className="email-input-container">
                  <span className="email-icon">✉️</span>
                  <input
                    type="email"
                    placeholder="Email address"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className="email-input"
                    required
                    autoComplete="email"
                  />
                </div>
              </div>

              <button type="submit" className="auth-submit-btn" disabled={loading}>
                {loading ? <span className="spinner spinner-sm" /> : 'Send Reset Link'}
              </button>
            </form>
          </div>

          <div className="auth-footer">
            <p>
              Remember your password?{' '}
              <Link to="/login" className="auth-link-forgot">Log in</Link>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
