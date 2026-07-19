import { useState } from 'react';
import { useSearchParams, Link, useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import magnetiteLogo from '../assets/magnetite-logo.svg';
import './auth.css';

const PASSWORD_LEVELS = [
  { min: 5, label: 'Very Strong', color: 'var(--color-success)' },
  { min: 4, label: 'Strong',      color: 'var(--color-success)' },
  { min: 3, label: 'Fair',        color: 'var(--color-warning)' },
  { min: 2, label: 'Weak',        color: 'var(--color-amber)' },
  { min: 1, label: 'Very Weak',   color: 'var(--color-error)' },
];

function getScore(pw) {
  let s = 0;
  if (pw.length >= 8)            s++;
  if (pw.length >= 12)           s++;
  if (/[A-Z]/.test(pw))         s++;
  if (/[0-9]/.test(pw))         s++;
  if (/[^A-Za-z0-9]/.test(pw))  s++;
  return s;
}

function PasswordStrengthBar({ password }) {
  const score = getScore(password);
  if (!password) return null;
  const level = PASSWORD_LEVELS.find((l) => score >= l.min) || PASSWORD_LEVELS[PASSWORD_LEVELS.length - 1];

  return (
    <div className="password-strength">
      <div className="strength-bars">
        {[1, 2, 3, 4, 5].map((seg) => (
          <div
            key={seg}
            className="strength-bar"
            style={{ backgroundColor: score >= seg ? level.color : undefined }}
          />
        ))}
      </div>
      <span className="strength-label" style={{ color: level.color }}>{level.label}</span>
    </div>
  );
}

export default function ResetPassword() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const token = searchParams.get('token');

  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);

  /* ── Invalid link ──────────────────────────────────────── */
  if (!token) {
    return (
      <div className="auth-page">
        <div className="auth-background" aria-hidden="true">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-state-card" role="alert">
              <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">✕</div>
              <div>
                <h1 className="auth-state-title">{t('auth.invalidLink')}</h1>
                <p className="auth-state-body">{t('auth.invalidResetLinkBody')}</p>
              </div>
              <Link to="/forgot-password" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
                {t('auth.requestNewLink')}
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
  }

  /* ── Success state ─────────────────────────────────────── */
  if (success) {
    return (
      <div className="auth-page">
        <div className="auth-background" aria-hidden="true">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-state-card" role="status" aria-live="polite">
              <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
              <div>
                <h1 className="auth-state-title">{t('auth.passwordReset')}</h1>
                <p className="auth-state-body">{t('auth.passwordResetBody')}</p>
              </div>
              <Link to="/login" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
                {t('auth.signInNow')}
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');

    if (password.length < 8) {
      setError(t('auth.passwordTooShort'));
      return;
    }
    if (password !== confirmPassword) {
      setError(t('auth.passwordsDoNotMatch'));
      return;
    }

    setLoading(true);
    try {
      await api.auth.resetPassword(token, password);
      setSuccess(true);
      setTimeout(() => navigate('/login'), 3000);
    } catch (err) {
      setError(err.message || t('auth.failedToResetPassword'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="auth-split">
      {/* Hero */}
      <div className="auth-hero" aria-hidden="true">
        <div className="auth-hero-glow" aria-hidden="true" />
        <div className="auth-hero-glow-amber" aria-hidden="true" />
        <div className="auth-hero-grain" aria-hidden="true" />
        <div className="auth-hero-content">
          <Link to="/" className="auth-hero-logo" tabIndex="-1">
            <img src={magnetiteLogo} className="auth-hero-logo-mark" aria-hidden="true" alt="" />
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>
          <div className="auth-hero-pitch reveal reveal-2">
            <span className="auth-hero-kicker">// SECURE YOUR ACCOUNT</span>
            <h1 className="auth-hero-heading">
              Choose a strong<br />
              <em>new password.</em>
            </h1>
            <p className="auth-hero-body">
              Use at least 8 characters with a mix of upper and lower case,
              numbers, and symbols for maximum security.
            </p>
          </div>
        </div>
      </div>

      {/* Form panel */}
      <div className="auth-form-panel">
        <div className="auth-form-inner">
          <div className="auth-panel-header reveal reveal-1">
            <span className="auth-kicker">// RESET PASSWORD</span>
            <h1 className="auth-title">{t('auth.newPasswordTitle')}</h1>
            <p className="auth-subtitle">{t('auth.newPasswordSubtitle')}</p>
          </div>

          <form
            onSubmit={handleSubmit}
            className="auth-form reveal reveal-2"
            aria-label={t('auth.resetPasswordFormLabel')}
            noValidate
          >
            {error && (
              <div className="auth-error" role="alert" aria-live="assertive">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <div className="input-wrapper">
              <label className="input-label" htmlFor="new-pw">{t('auth.newPasswordLabel')}</label>
              <input
                id="new-pw"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="input-field"
                placeholder={t('auth.newPasswordPlaceholder')}
                required
                autoComplete="new-password"
                aria-required="true"
              />
              <PasswordStrengthBar password={password} />
            </div>

            <div className="input-wrapper">
              <label className="input-label" htmlFor="confirm-pw">{t('auth.confirmPasswordLabel')}</label>
              <input
                id="confirm-pw"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="input-field"
                placeholder={t('auth.confirmPasswordPlaceholder')}
                required
                autoComplete="new-password"
                aria-required="true"
              />
            </div>

            <button
              type="submit"
              className="auth-submit-btn"
              disabled={loading}
              aria-busy={loading}
            >
              {loading
                ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('auth.resettingPassword')}</span></>
                : t('auth.resetPasswordBtn')}
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
