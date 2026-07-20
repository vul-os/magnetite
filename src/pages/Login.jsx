import { useId, useState } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { getOAuthUrl } from '../api/client';
import OAuthButtons from '../components/auth/OAuthButtons';
import { useTranslation } from '../i18n/useTranslation';
import magnetiteLogo from '../assets/magnetite-logo.svg';
import './auth.css';

/**
 * Login — the auth exemplar.
 *
 * Form pattern established here (copy it for Register / ForgotPassword /
 * ResetPassword):
 *   .form-group > .form-label[htmlFor] + .form-input[id]
 *   a single form-level .auth-alert[role=alert] whose id is referenced by
 *   aria-describedby on every field it invalidates, plus aria-invalid on
 *   those fields.
 *   submit is .btn.btn-primary with aria-busy + a live sr-only status.
 *
 * Nothing on this page asserts a number we cannot source. The left panel
 * states properties of the platform that are true of the code in this repo
 * (deterministic sim, replay verification, keypair identity, MIT licence).
 */
export default function Login() {
  const { login } = useAuth();
  const { t } = useTranslation();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(null);

  // Stable, collision-free ids: this page may be rendered more than once in a
  // test tree, and duplicate ids are an accessibility violation.
  const uid = useId();
  const emailId = `${uid}-email`;
  const passwordId = `${uid}-password`;
  const errorId = `${uid}-error`;
  const statusId = `${uid}-status`;

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

    // Generate a CSRF state nonce and persist it in sessionStorage so
    // AuthCallback.jsx can validate that the OAuth provider echoed the same value.
    // crypto.randomUUID() is available in all modern browsers and in the test env.
    const stateNonce = crypto.randomUUID();
    sessionStorage.setItem('oauth_state_nonce', stateNonce);

    const oauthUrl = new URL(getOAuthUrl(provider));
    oauthUrl.searchParams.set('state', stateNonce);
    window.location.href = oauthUrl.toString();
  };

  // Both credential fields are implicated by a failed sign-in, so both point at
  // the same alert. Colour is never the only signal: the alert carries a text
  // heading and the .form-error/alert marker glyph.
  const describedBy = error ? errorId : undefined;

  return (
    <div className="auth-split">
      {/* ── Instrument panel ─────────────────────────────────────── */}
      <aside className="auth-hero">
        <div className="auth-hero-field" aria-hidden="true" />

        <div className="auth-hero-content">
          <Link to="/" className="auth-hero-logo">
            <img src={magnetiteLogo} className="auth-hero-logo-mark" alt="" aria-hidden="true" />
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>

          <div className="auth-hero-pitch">
            <span className="kicker">// DECENTRALIZED GAME PLATFORM</span>
            <p className="auth-hero-heading">
              A game is a portable object.<br />
              A node is generic compute.
            </p>
            <p className="auth-hero-body">
              Run the node binary anywhere. Identity is a keypair, the simulation is
              deterministic, and every match can be re-simulated by anyone who has the
              replay. There is no cloud in the middle.
            </p>
          </div>

          <dl className="auth-hero-props">
            <div className="auth-hero-prop edge-field">
              <dt className="m-sm">Simulation</dt>
              <dd>Deterministic and replay-verifiable</dd>
            </div>
            <div className="auth-hero-prop edge-field">
              <dt className="m-sm">Identity</dt>
              <dd>Keypair, held by you</dd>
            </div>
            <div className="auth-hero-prop edge-field">
              <dt className="m-sm">Licence</dt>
              <dd>MIT, source in this repository</dd>
            </div>
          </dl>
        </div>
      </aside>

      {/* ── Form panel ───────────────────────────────────────────── */}
      <main className="auth-form-panel">
        <div className="auth-form-inner">
          <header className="auth-panel-header reveal reveal-1">
            <span className="kicker">// SIGN IN</span>
            <h1 className="auth-title">{t('auth.loginTitle')}</h1>
            <p className="auth-subtitle">{t('auth.loginSubtitle')}</p>
          </header>

          <div className="reveal reveal-2">
            <OAuthButtons
              layout="vertical"
              loadingProvider={oauthLoading}
              onProviderClick={handleOAuth}
            />
          </div>

          <div className="auth-divider reveal reveal-3">
            <span>{t('auth.orContinueWithEmail')}</span>
          </div>

          <form
            onSubmit={handleSubmit}
            className="auth-form reveal reveal-4"
            aria-label={t('auth.signInFormLabel')}
            noValidate
          >
            {error && (
              <div className="auth-alert" id={errorId} role="alert">
                <span className="auth-alert-mark" aria-hidden="true">!</span>
                <div className="auth-alert-body">
                  <strong className="auth-alert-title">Sign-in failed</strong>
                  <span className="auth-alert-text">{error}</span>
                </div>
              </div>
            )}

            <div className="form-group">
              <label className="form-label" htmlFor={emailId}>
                {t('auth.emailLabel')}
              </label>
              <input
                id={emailId}
                className="form-input mono"
                type="email"
                name="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder={t('auth.emailPlaceholder')}
                autoComplete="email"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck="false"
                aria-invalid={error ? 'true' : undefined}
                aria-describedby={describedBy}
                required
              />
            </div>

            <div className="form-group">
              <label className="form-label" htmlFor={passwordId}>
                {t('auth.passwordLabel')}
              </label>
              <div className="auth-input-affix">
                <input
                  id={passwordId}
                  className="form-input mono"
                  type={showPassword ? 'text' : 'password'}
                  name="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={t('auth.passwordPlaceholder')}
                  autoComplete="current-password"
                  aria-invalid={error ? 'true' : undefined}
                  aria-describedby={describedBy}
                  required
                />
                <button
                  type="button"
                  className="auth-affix-btn"
                  onClick={() => setShowPassword((v) => !v)}
                  aria-label={showPassword ? 'Hide password' : 'Show password'}
                  aria-pressed={showPassword}
                  aria-controls={passwordId}
                >
                  <span aria-hidden="true">{showPassword ? 'HIDE' : 'SHOW'}</span>
                </button>
              </div>
            </div>

            <div className="auth-form-footer">
              <Link to="/forgot-password" className="auth-link-forgot">
                {t('auth.forgotPassword')}
              </Link>
            </div>

            <button
              type="submit"
              className="btn btn-primary btn-lg btn-block auth-submit"
              disabled={loading}
              aria-busy={loading}
              aria-describedby={statusId}
            >
              {loading && <span className="spinner spinner-sm" aria-hidden="true" />}
              {loading ? t('auth.signingIn') : t('auth.signInBtn')}
            </button>

            {/* Politely announced busy state — the button label alone changes
                too late for some screen readers. */}
            <p className="sr-only" id={statusId} role="status">
              {loading ? t('auth.signingIn') : ''}
            </p>
          </form>

          <p className="auth-switch reveal reveal-5">
            <span>{t('auth.noAccountPrompt')}</span>{' '}
            <Link to="/register" className="auth-switch-link">
              {t('auth.createAccountLink')}
            </Link>
          </p>
        </div>
      </main>
    </div>
  );
}
