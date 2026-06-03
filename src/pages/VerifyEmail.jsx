import { useState, useEffect } from 'react';
import { useSearchParams, Link } from 'react-router-dom';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
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
  const { t } = useTranslation();
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
      setError(err.message || t('auth.failedToResendVerification'));
    } finally {
      setResending(false);
    }
  };

  if (status === 'loading') {
    return (
      <AuthShell>
        <div className="auth-state-card" aria-live="polite" role="status">
          <div className="auth-state-icon auth-state-icon--processing" aria-label={t('auth.verifyingEmail')}>
            <span className="spinner spinner-md" style={{ color: 'var(--color-accent)' }} />
          </div>
          <div>
            <h1 className="auth-state-title">{t('auth.verifyingEmailTitle')}</h1>
            <p className="auth-state-body">{t('auth.verifyingEmailBody')}</p>
          </div>
        </div>
      </AuthShell>
    );
  }

  if (status === 'invalid') {
    return (
      <AuthShell>
        <div className="auth-state-card" role="alert">
          <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">✕</div>
          <div>
            <h1 className="auth-state-title">{t('auth.invalidLink')}</h1>
            <p className="auth-state-body">{t('auth.invalidVerificationLinkBody')}</p>
          </div>
          <Link to="/register" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
            {t('auth.signUpLink')}
          </Link>
        </div>
      </AuthShell>
    );
  }

  if (status === 'success') {
    return (
      <AuthShell>
        <div className="auth-state-card" role="status" aria-live="polite">
          <div className="auth-state-icon auth-state-icon--success" aria-hidden="true">✓</div>
          <div>
            <h1 className="auth-state-title">{t('auth.emailVerified')}</h1>
            <p className="auth-state-body">{t('auth.emailVerifiedBody')}</p>
          </div>
          <Link to="/login" className="auth-submit-btn" style={{ textDecoration: 'none' }}>
            {t('auth.signInLink')}
          </Link>
        </div>
      </AuthShell>
    );
  }

  /* error state */
  return (
    <AuthShell>
      <div className="auth-state-card" role="alert">
        <div className="auth-state-icon auth-state-icon--error" aria-hidden="true">!</div>
        <div>
          <h1 className="auth-state-title">{t('auth.verificationFailed')}</h1>
          <p className="auth-state-body">
            {error || t('auth.verificationLinkExpired')}
          </p>
        </div>

        <button
          className="auth-submit-btn"
          onClick={handleResend}
          disabled={resending || resent}
          style={{ width: '100%' }}
          aria-busy={resending}
        >
          {resending
            ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('auth.resendingVerification')}</span></>
            : resent
              ? t('auth.emailResentSuccess')
              : t('auth.resendVerificationEmail')}
        </button>

        {resent && (
          <div className="auth-success" role="status" aria-live="polite">
            <span className="auth-success-icon" aria-hidden="true">✓</span>
            {t('auth.checkInboxForNewLink')}
          </div>
        )}

        {error && !resent && (
          <div className="auth-error" role="alert" aria-live="assertive">
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {error}
          </div>
        )}

        <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: 0 }}>
          {t('auth.alreadyVerifiedPrompt')}{' '}
          <Link to="/login" className="auth-link-forgot">{t('auth.signInLink')}</Link>
        </p>
      </div>
    </AuthShell>
  );
}
