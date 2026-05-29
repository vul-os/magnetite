import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { getOAuthUrl } from '../api/client';
import OAuthButtons from '../components/auth/OAuthButtons';
import EmailInput from '../components/auth/EmailInput';
import PasswordInput from '../components/auth/PasswordInput';
import TermsCheckbox from '../components/auth/TermsCheckbox';
import SocialProof from '../components/auth/SocialProof';
import './auth.css';

export default function Register() {
  const { register } = useAuth();
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [termsAccepted, setTermsAccepted] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(null);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    if (!termsAccepted) {
      setError('You must accept the terms and conditions');
      return;
    }
    setLoading(true);
    try {
      await register(username, email, password);
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
          <Link to="/" className="auth-hero-logo">
            <div className="auth-hero-logo-mark">M</div>
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>

          <div className="auth-hero-pitch reveal reveal-2">
            <span className="auth-hero-kicker">// START BUILDING</span>
            <h1 className="auth-hero-heading">
              Your first Rust game,<br />
              <em>live in minutes.</em>
            </h1>
            <p className="auth-hero-body">
              Magnetite gives you server infrastructure, matchmaking, WASM builds,
              and USDC payments — so you can focus entirely on your game logic.
            </p>
            <div className="auth-hero-stats reveal reveal-3">
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">0</span>
                <span className="auth-hero-stat-label">infra config</span>
              </div>
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">85%</span>
                <span className="auth-hero-stat-label">dev revenue</span>
              </div>
              <div className="auth-hero-stat">
                <span className="auth-hero-stat-value">MIT</span>
                <span className="auth-hero-stat-label">open source</span>
              </div>
            </div>
          </div>

          <div className="auth-hero-features reveal reveal-4">
            {[
              'Bevy + WASM build pipeline, zero config',
              'Playtime-based developer payouts in USDC',
              'Scale to AAA with the same SDK',
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
          <div className="auth-panel-header reveal reveal-1">
            <span className="auth-kicker">// CREATE ACCOUNT</span>
            <h1 className="auth-title">Join Magnetite</h1>
            <p className="auth-subtitle">Build and ship Rust games to thousands of players</p>
          </div>

          <div className="reveal reveal-2">
            <OAuthButtons
              layout="vertical"
              loadingProvider={oauthLoading}
              onProviderClick={handleOAuth}
            />
          </div>

          <div className="auth-divider reveal reveal-3">
            <span>or sign up with email</span>
          </div>

          <form onSubmit={handleSubmit} className="auth-form reveal reveal-4">
            {error && (
              <div className="auth-error" role="alert">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <div className="input-wrapper">
              <label className="input-label" htmlFor="reg-username">Username</label>
              <input
                id="reg-username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="input-field"
                placeholder="Choose a username"
                required
                minLength={3}
                maxLength={20}
                autoComplete="username"
              />
            </div>

            <EmailInput
              value={email}
              onChange={setEmail}
              placeholder="Email address"
            />

            <PasswordInput
              value={password}
              onChange={setPassword}
              placeholder="Create a password"
              showStrength
              showRequirements
            />

            <TermsCheckbox
              checked={termsAccepted}
              onChange={setTermsAccepted}
              required
            />

            <button
              type="submit"
              className="auth-submit-btn"
              disabled={!termsAccepted || loading}
            >
              {loading
                ? <span className="spinner spinner-sm" aria-hidden="true" />
                : 'Create Account'}
            </button>
          </form>

          <div className="auth-switch reveal reveal-5">
            <span>Already have an account?</span>
            <Link to="/login" className="auth-switch-link">Sign in</Link>
          </div>

          <div className="reveal reveal-6">
            <SocialProof />
          </div>
        </div>
      </div>
    </div>
  );
}
