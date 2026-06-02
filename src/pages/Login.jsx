import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { getOAuthUrl } from '../api/client';
import OAuthButtons from '../components/auth/OAuthButtons';
import EmailInput from '../components/auth/EmailInput';
import PasswordInput from '../components/auth/PasswordInput';
import SocialProof from '../components/auth/SocialProof';
import './auth.css';

export default function Login() {
  const { login } = useAuth();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(null);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await login(email, password);
    } catch (err) {
      setError(err.message);
      setLoading(false);
    }
  };

  const handleOAuth = (provider) => {
    setOauthLoading(provider);
    setError('');
    window.location.href = getOAuthUrl(provider);
  };

  return (
    <div className="auth-split">
      {/* ── Hero Panel ──────────────────────────────────────── */}
      <div className="auth-hero">
        <div className="auth-hero-glow" aria-hidden="true" />
        <div className="auth-hero-glow-amber" aria-hidden="true" />
        <div className="auth-hero-grain" aria-hidden="true" />

        <div className="auth-hero-content">
          {/* Logo */}
          <Link to="/" className="auth-hero-logo">
            <div className="auth-hero-logo-mark">M</div>
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>

          {/* Pitch */}
          <div className="auth-hero-pitch reveal reveal-2">
            <span className="auth-hero-kicker">// BUILT IN RUST</span>
            <h1 className="auth-hero-heading">
              Ship games that<br />
              <em>scale to AAA.</em>
            </h1>
            <p className="auth-hero-body">
              The open-source platform for building, distributing, and monetising
              Rust games — from weekend game jam to a live-service title with
              millions of players.
            </p>
            <div className="auth-hero-stats reveal reveal-3">
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">15%</span>
                <span className="auth-hero-stat-label">platform fee</span>
              </div>
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">USD</span>
                <span className="auth-hero-stat-label">Wise payouts</span>
              </div>
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">MIT</span>
                <span className="auth-hero-stat-label">open source</span>
              </div>
            </div>
          </div>

          {/* Feature bullets */}
          <div className="auth-hero-features reveal reveal-4">
            {[
              'Server-authoritative netcode, zero boilerplate',
              'WASM + native targets from one Rust codebase',
              'Matchmaking, persistence & analytics built-in',
            ].map((f) => (
              <div key={f} className="auth-hero-feature">
                <span className="auth-hero-feature-dot" aria-hidden="true" />
                {f}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* ── Form Panel ─────────────────────────────────────── */}
      <div className="auth-form-panel">
        <div className="auth-form-inner">
          {/* Panel header */}
          <div className="auth-panel-header reveal reveal-1">
            <span className="auth-kicker">// SIGN IN</span>
            <h1 className="auth-title">Welcome back</h1>
            <p className="auth-subtitle">Sign in to continue to your Magnetite account</p>
          </div>

          {/* OAuth */}
          <div className="reveal reveal-2">
            <OAuthButtons
              layout="vertical"
              loadingProvider={oauthLoading}
              onProviderClick={handleOAuth}
            />
          </div>

          <div className="auth-divider reveal reveal-3">
            <span>or continue with email</span>
          </div>

          {/* Email / password form */}
          <form onSubmit={handleSubmit} className="auth-form reveal reveal-4">
            {error && (
              <div className="auth-error" role="alert">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <EmailInput
              value={email}
              onChange={setEmail}
              placeholder="Email address"
            />

            <PasswordInput
              value={password}
              onChange={setPassword}
              placeholder="Password"
              showStrength={false}
            />

            <div className="auth-form-footer">
              <Link to="/forgot-password" className="auth-link auth-link-forgot">
                Forgot password?
              </Link>
            </div>

            <button type="submit" className="auth-submit-btn" disabled={loading}>
              {loading ? <span className="spinner spinner-sm" aria-hidden="true" /> : 'Sign In'}
            </button>
          </form>

          {/* Switch */}
          <div className="auth-switch reveal reveal-5">
            <span>Don&apos;t have an account?</span>
            <Link to="/register" className="auth-switch-link">Create account</Link>
          </div>

          <div className="reveal reveal-6">
            <SocialProof />
          </div>
        </div>
      </div>
    </div>
  );
}
