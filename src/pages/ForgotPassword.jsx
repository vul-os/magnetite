import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './auth.css';

export default function ForgotPassword() {
  const { t } = useTranslation();
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
      setError(err.message || t('auth.failedToSendResetLink'));
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
            <div className="auth-state-card" role="status" aria-live="polite">
              <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
              <div>
                <h1 className="auth-state-title">{t('auth.checkInbox')}</h1>
                <p className="auth-state-body">
                  {t('auth.resetLinkSentTo')}{' '}
                  <strong style={{ color: 'var(--color-accent)' }}>{email}</strong>.
                  {' '}{t('auth.resetLinkExpiry')}
                </p>
              </div>
              <Link
                to="/login"
                className="auth-submit-btn"
                style={{ textDecoration: 'none' }}
              >
                {t('auth.backToSignIn')}
              </Link>
              <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: 0 }}>
                {t('auth.didntReceiveIt')}{' '}
                <button
                  className="auth-link auth-link-forgot"
                  style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: 'inherit', padding: 0 }}
                  onClick={() => setSuccess(false)}
                >
                  {t('auth.tryAgainLink')}
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
      <div className="auth-hero" aria-hidden="true">
        <div className="auth-hero-glow" aria-hidden="true" />
        <div className="auth-hero-glow-amber" aria-hidden="true" />
        <div className="auth-hero-grain" aria-hidden="true" />
        <div className="auth-hero-content">
          <Link to="/" className="auth-hero-logo" tabIndex="-1">
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
            <h1 className="auth-title">{t('auth.resetPassword')}</h1>
            <p className="auth-subtitle">{t('auth.forgotPasswordSubtitle')}</p>
          </div>

          <form
            onSubmit={handleSubmit}
            className="auth-form reveal reveal-2"
            aria-label={t('auth.forgotPasswordFormLabel')}
            noValidate
          >
            {error && (
              <div className="auth-error" role="alert" aria-live="assertive">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <div className="email-input-wrapper">
              <label className="input-label" htmlFor="forgot-email">{t('auth.emailLabel')}</label>
              <div className="email-input-container">
                <span className="email-icon" aria-hidden="true">✉</span>
                <input
                  type="email"
                  id="forgot-email"
                  placeholder={t('auth.emailPlaceholder')}
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="email-input"
                  required
                  autoComplete="email"
                  aria-required="true"
                />
              </div>
            </div>

            <button
              type="submit"
              className="auth-submit-btn"
              disabled={loading}
              aria-busy={loading}
            >
              {loading
                ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('auth.sendingResetLink')}</span></>
                : t('auth.sendResetLink')}
            </button>
          </form>

          <div className="auth-switch reveal reveal-3">
            <span>{t('auth.rememberPasswordPrompt')}</span>
            <Link to="/login" className="auth-switch-link">{t('auth.signInLink')}</Link>
          </div>
        </div>
      </div>
    </div>
  );
}
