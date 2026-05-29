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

  /* ── Success state ─────────────────────────────────────── */
  if (success) {
    return (
      <div className="auth-page">
        <div className="auth-background" aria-hidden="true">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
          <div className="auth-bg-glow auth-bg-glow-2" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-state-card">
              <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
              <div>
                <h1 className="auth-state-title">Check your inbox</h1>
                <p className="auth-state-body">
                  We sent a password reset link to{' '}
                  <strong style={{ color: 'var(--color-accent)' }}>{email}</strong>.
                  The link expires in 1 hour.
                </p>
              </div>
              <Link
                to="/login"
                className="auth-submit-btn"
                style={{ textDecoration: 'none' }}
              >
                Back to Sign In
              </Link>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: 0 }}>
                Didn&apos;t receive it? Check your spam folder or{' '}
                <button
                  className="auth-link auth-link-forgot"
                  style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: 'inherit', padding: 0 }}
                  onClick={() => setSuccess(false)}
                >
                  try again
                </button>
              </p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  /* ── Form ──────────────────────────────────────────────── */
  return (
    <div className="auth-split">
      {/* Hero */}
      <div className="auth-hero">
        <div className="auth-hero-glow" aria-hidden="true" />
        <div className="auth-hero-glow-amber" aria-hidden="true" />
        <div className="auth-hero-grain" aria-hidden="true" />
        <div className="auth-hero-content">
          <Link to="/" className="auth-hero-logo">
            <div className="auth-hero-logo-mark">M</div>
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>
          <div className="auth-hero-pitch reveal reveal-2">
            <span className="auth-hero-kicker">// ACCOUNT RECOVERY</span>
            <h1 className="auth-hero-heading">
              We&apos;ll get you<br />
              <em>back in.</em>
            </h1>
            <p className="auth-hero-body">
              Enter your email and we&apos;ll send a secure reset link valid for one hour.
              Your game data and wallet balance are safe.
            </p>
          </div>
          <div className="auth-hero-features reveal reveal-3">
            {[
              'Secure, time-limited reset link',
              'Your wallet and game data remain intact',
              'Contact support if you need more help',
            ].map((f) => (
              <div key={f} className="auth-hero-feature">
                <span className="auth-hero-feature-dot" aria-hidden="true" />
                {f}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Form panel */}
      <div className="auth-form-panel">
        <div className="auth-form-inner">
          <div className="auth-panel-header reveal reveal-1">
            <span className="auth-kicker">// FORGOT PASSWORD</span>
            <h1 className="auth-title">Reset password</h1>
            <p className="auth-subtitle">
              Enter your email and we&apos;ll send you a reset link
            </p>
          </div>

          <form onSubmit={handleSubmit} className="auth-form reveal reveal-2">
            {error && (
              <div className="auth-error" role="alert">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <div className="email-input-wrapper">
              <div className="email-input-container">
                <span className="email-icon" aria-hidden="true">✉</span>
                <input
                  type="email"
                  id="forgot-email"
                  placeholder="Email address"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="email-input"
                  required
                  autoComplete="email"
                  aria-label="Email address"
                />
              </div>
            </div>

            <button type="submit" className="auth-submit-btn" disabled={loading}>
              {loading
                ? <span className="spinner spinner-sm" aria-hidden="true" />
                : 'Send Reset Link'}
            </button>
          </form>

          <div className="auth-switch reveal reveal-3">
            <span>Remember your password?</span>
            <Link to="/login" className="auth-switch-link">Sign in</Link>
          </div>
        </div>
      </div>
    </div>
  );
}
