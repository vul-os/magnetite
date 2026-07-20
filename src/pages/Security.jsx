import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

function authFetch(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  return fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });
}

/* Mock fallbacks — only used when VITE_USE_MOCKS=true */
const MOCK_SESSIONS = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 'sess_001', device: 'Chrome on Mac OS',   location: 'San Francisco, CA', lastActive: 'Now',        current: true  },
      { id: 'sess_002', device: 'Safari on iPhone',   location: 'San Francisco, CA', lastActive: '2 hours ago', current: false },
      { id: 'sess_003', device: 'Firefox on Windows', location: 'New York, NY',       lastActive: 'Yesterday',  current: false },
    ]
  : null;

function normaliseSession(s) {
  return {
    id:         s.id ?? s.session_id,
    device:     s.user_agent ?? s.device ?? 'Unknown device',
    location:   s.ip_address ?? s.location ?? 'Unknown',
    lastActive: s.last_active ?? s.updated_at
      ? new Date(s.last_active ?? s.updated_at).toLocaleString()
      : 'Unknown',
    current:    s.current ?? false,
  };
}

function normaliseApiKey(k) {
  return {
    id:          k.id,
    name:        k.name ?? 'Unnamed key',
    prefix:      k.prefix ?? k.key_prefix ?? (k.key ? k.key.slice(0, 8) + '...' : '••••••••'),
    createdAt:   k.created_at ? new Date(k.created_at).toLocaleDateString() : '—',
    lastUsed:    k.last_used_at ? new Date(k.last_used_at).toLocaleDateString() : 'Never',
  };
}

export default function Security() {
  const { t } = useTranslation();
  const [passwords, setPasswords]           = useState({ current: '', new: '', confirm: '' });
  const [passwordError, setPasswordError]   = useState('');
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [changingPw, setChangingPw]         = useState(false);

  // ── 2FA ──────────────────────────────────────────────────────────────────────
  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [showSetup2FA, setShowSetup2FA]     = useState(false);
  const [otpauthUri, setOtpauthUri]         = useState('');
  const [qrDataUrl, setQrDataUrl]           = useState('');
  const [twoFaCode, setTwoFaCode]           = useState('');
  const [twoFaError, setTwoFaError]         = useState('');
  const [twoFaLoading, setTwoFaLoading]     = useState(false);
  const [disableCode, setDisableCode]       = useState('');
  const [showDisableForm, setShowDisableForm] = useState(false);

  // ── Sessions ──────────────────────────────────────────────────────────────────
  const [sessions, setSessions]             = useState(MOCK_SESSIONS ?? []);
  const [sessionsLoading, setSessionsLoading] = useState(!MOCK_SESSIONS);
  const [sessionError, setSessionError]     = useState(null);

  // ── API Keys ──────────────────────────────────────────────────────────────────
  const [apiKeys, setApiKeys]               = useState([]);
  const [apiKeysLoading, setApiKeysLoading] = useState(false);
  const [apiKeysError, setApiKeysError]     = useState(null);
  const [newKeyName, setNewKeyName]         = useState('');
  const [showNewKeyForm, setShowNewKeyForm] = useState(false);
  const [createdKey, setCreatedKey]         = useState(null); // one-time display
  const [creatingKey, setCreatingKey]       = useState(false);
  const [revokingId, setRevokingId]         = useState(null);

  /* Load real sessions from backend */
  useEffect(() => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    async function loadSessions() {
      setSessionsLoading(true);
      try {
        const res = await authFetch('/api/auth/sessions');
        if (res.ok) {
          const data = await res.json();
          const raw = data.sessions ?? data ?? [];
          setSessions(Array.isArray(raw) ? raw.map(normaliseSession) : []);
        }
      } catch {
        /* leave empty — not a blocking error */
      } finally {
        setSessionsLoading(false);
      }
    }
    loadSessions();
  }, []);

  /* Load API keys */
  const loadApiKeys = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    setApiKeysLoading(true);
    setApiKeysError(null);
    try {
      const data = await api.auth.apiKeys();
      const raw = data?.keys ?? data ?? [];
      setApiKeys(Array.isArray(raw) ? raw.map(normaliseApiKey) : []);
    } catch (err) {
      if (!err.message?.includes('404') && !err.message?.includes('not found')) {
        setApiKeysError(err.message || 'Failed to load API keys');
      }
      // 404 = endpoint not yet deployed; silently show empty
    } finally {
      setApiKeysLoading(false);
    }
  }, []);

  // Load API keys from the API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { loadApiKeys(); }, [loadApiKeys]);

  const handlePasswordChange = async (e) => {
    e.preventDefault();
    setPasswordError('');
    setPasswordSuccess(false);

    if (passwords.new !== passwords.confirm) {
      setPasswordError(t('auth.passwordsDoNotMatch'));
      return;
    }
    if (passwords.new.length < 8) {
      setPasswordError(t('auth.passwordTooShort'));
      return;
    }
    if (!passwords.current) {
      setPasswordError(t('account.currentPasswordRequired'));
      return;
    }

    setChangingPw(true);
    try {
      await api.auth.updatePassword(passwords.current, passwords.new);
      setPasswordSuccess(true);
      setPasswords({ current: '', new: '', confirm: '' });
      setTimeout(() => setPasswordSuccess(false), 3000);
    } catch (err) {
      setPasswordError(err.message || t('account.failedToUpdatePassword'));
    } finally {
      setChangingPw(false);
    }
  };

  // ── 2FA handlers ──────────────────────────────────────────────────────────────

  const handleBeginSetup2FA = async () => {
    setTwoFaError('');
    setTwoFaLoading(true);
    try {
      const data = await api.auth.setup2fa();
      setOtpauthUri(data.otpauth_uri ?? data.uri ?? '');
      setQrDataUrl(data.qr_data_url ?? data.qr_url ?? '');
      setShowSetup2FA(true);
    } catch (err) {
      if (err.message?.includes('404') || err.message?.includes('not found') || err.message?.includes('not yet')) {
        setTwoFaError('2FA setup endpoint not yet deployed on this server. The backend route will be added soon.');
      } else {
        setTwoFaError(err.message || '2FA setup failed');
      }
    } finally {
      setTwoFaLoading(false);
    }
  };

  const handleVerify2FA = async (e) => {
    e.preventDefault();
    setTwoFaLoading(true);
    setTwoFaError('');
    try {
      await api.auth.verify2fa(twoFaCode);
      setTwoFactorEnabled(true);
      setShowSetup2FA(false);
      setTwoFaCode('');
      setOtpauthUri('');
      setQrDataUrl('');
    } catch (err) {
      setTwoFaError(err.message || 'Verification failed — check the code and try again');
    } finally {
      setTwoFaLoading(false);
    }
  };

  const handleDisable2FA = async (e) => {
    e.preventDefault();
    setTwoFaLoading(true);
    setTwoFaError('');
    try {
      await api.auth.disable2fa(disableCode);
      setTwoFactorEnabled(false);
      setShowDisableForm(false);
      setDisableCode('');
    } catch (err) {
      setTwoFaError(err.message || 'Could not disable 2FA — check your code and try again');
    } finally {
      setTwoFaLoading(false);
    }
  };

  const handleSignOutAll = async () => {
    setSessionError(null);
    try {
      const res = await authFetch('/api/auth/sessions/all', { method: 'DELETE' });
      if (res.ok) {
        setSessions(prev => prev.filter(s => s.current));
      } else {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || 'Failed to sign out all sessions');
      }
    } catch (err) {
      setSessionError(err.message);
    }
  };

  const handleRevokeSession = async (sessionId) => {
    setSessionError(null);
    setSessions(prev => prev.filter(s => s.id !== sessionId));
  };

  // ── API Key handlers ──────────────────────────────────────────────────────────

  const handleCreateApiKey = async (e) => {
    e.preventDefault();
    if (!newKeyName.trim()) return;
    setCreatingKey(true);
    setApiKeysError(null);
    try {
      const data = await api.auth.createApiKey(newKeyName.trim());
      // Store the one-time plaintext key for display before clearing
      setCreatedKey({ id: data.id, name: data.name ?? newKeyName, key: data.key });
      setShowNewKeyForm(false);
      setNewKeyName('');
      // Reload the key list (will not show plaintext key again)
      await loadApiKeys();
    } catch (err) {
      setApiKeysError(err.message || 'Failed to create API key');
    } finally {
      setCreatingKey(false);
    }
  };

  const handleRevokeApiKey = async (id) => {
    setRevokingId(id);
    try {
      await api.auth.revokeApiKey(id);
      setApiKeys(prev => prev.filter(k => k.id !== id));
    } catch (err) {
      setApiKeysError(err.message || 'Failed to revoke API key');
    } finally {
      setRevokingId(null);
    }
  };

  return (
    <Layout>
      <div className="security-page">
        {/* Header */}
        <header className="settings-page-header reveal reveal-1">
          <span className="kicker">// SECURITY</span>
          <h1 className="settings-page-title">{t('account.securityTitle')}</h1>
          <p className="settings-page-subtitle">{t('account.securitySubtitle')}</p>
        </header>

        {/* Password */}
        <section className="settings-section reveal reveal-2" aria-labelledby="sec-pw-heading">
          <h2 id="sec-pw-heading" className="settings-section-title">{t('account.changePassword')}</h2>
          <p className="settings-section-desc">{t('account.changePasswordDesc')}</p>

          <form onSubmit={handlePasswordChange} aria-label={t('account.changePasswordFormLabel')} noValidate>
            <div className="settings-field">
              <label className="settings-field-label" htmlFor="sec-cur-pw">{t('account.currentPassword')}</label>
              <input
                id="sec-cur-pw"
                type="password"
                value={passwords.current}
                onChange={(e) => setPasswords({ ...passwords, current: e.target.value })}
                className="settings-input"
                placeholder={t('account.enterCurrentPassword')}
                autoComplete="current-password"
              />
            </div>
            <div className="settings-grid-2">
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="sec-new-pw">{t('auth.newPasswordLabel')}</label>
                <input
                  id="sec-new-pw"
                  type="password"
                  value={passwords.new}
                  onChange={(e) => setPasswords({ ...passwords, new: e.target.value })}
                  className="settings-input"
                  placeholder={t('auth.newPasswordPlaceholder')}
                  autoComplete="new-password"
                />
              </div>
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="sec-conf-pw">{t('auth.confirmPasswordLabel')}</label>
                <input
                  id="sec-conf-pw"
                  type="password"
                  value={passwords.confirm}
                  onChange={(e) => setPasswords({ ...passwords, confirm: e.target.value })}
                  className="settings-input"
                  placeholder={t('auth.confirmPasswordPlaceholder')}
                  autoComplete="new-password"
                />
              </div>
            </div>

            {passwordError && (
              <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {passwordError}
              </div>
            )}
            {passwordSuccess && (
              <div className="auth-success" role="status" aria-live="polite" style={{ marginBottom: '1rem' }}>
                <span className="auth-success-icon" aria-hidden="true">✓</span>
                {t('account.passwordUpdatedSuccess')}
              </div>
            )}

            <button type="submit" className="settings-save-btn" disabled={changingPw} aria-busy={changingPw}>
              {changingPw ? (
                <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('account.updatingPassword')}</span></>
              ) : t('account.updatePassword')}
            </button>
          </form>
        </section>

        {/* 2FA */}
        <section className="settings-section reveal reveal-3" aria-labelledby="sec-2fa-heading">
          <h2 id="sec-2fa-heading" className="settings-section-title">{t('account.twoFactorAuth')}</h2>
          <p className="settings-section-desc">{t('account.twoFactorAuthDesc')}</p>

          {twoFaError && (
            <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {twoFaError}
            </div>
          )}

          {twoFactorEnabled ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="twofa-status-enabled">
                <span aria-hidden="true">✓</span>
                {t('account.twoFAEnabled')}
              </div>
              {showDisableForm ? (
                <form onSubmit={handleDisable2FA} style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem', maxWidth: 320 }} aria-label={t('account.disable2FAFormLabel')}>
                  <div className="settings-field">
                    <label className="settings-field-label" htmlFor="disable-2fa-code">
                      {t('account.disable2FACodeLabel')}
                    </label>
                    <input
                      id="disable-2fa-code"
                      type="text"
                      className="settings-input"
                      placeholder="000000"
                      maxLength={6}
                      pattern="[0-9]{6}"
                      inputMode="numeric"
                      autoComplete="one-time-code"
                      value={disableCode}
                      onChange={(e) => setDisableCode(e.target.value)}
                      style={{ fontFamily: 'var(--font-mono)', letterSpacing: '0.2em' }}
                    />
                  </div>
                  <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                    <button type="submit" className="settings-action-btn danger" disabled={twoFaLoading} aria-busy={twoFaLoading}>
                      {twoFaLoading ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('account.disabling2FA')}</span></> : t('account.confirmDisable')}
                    </button>
                    <button type="button" className="settings-action-btn" onClick={() => { setShowDisableForm(false); setDisableCode(''); setTwoFaError(''); }}>
                      {t('common.cancel')}
                    </button>
                  </div>
                </form>
              ) : (
                <button
                  className="settings-action-btn"
                  onClick={() => setShowDisableForm(true)}
                  style={{ alignSelf: 'flex-start' }}
                >
                  {t('account.disable2FA')}
                </button>
              )}
            </div>
          ) : showSetup2FA ? (
            <form onSubmit={handleVerify2FA} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }} aria-label={t('account.setup2FAFormLabel')}>
              <div className="twofa-qr-placeholder">
                {qrDataUrl ? (
                  <img
                    src={qrDataUrl}
                    alt={t('account.scanQRCode')}
                    style={{ width: 160, height: 160, imageRendering: 'pixelated' }}
                  />
                ) : otpauthUri ? (
                  <div style={{ maxWidth: 340 }}>
                    <p style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', wordBreak: 'break-all', color: 'var(--color-accent)', background: 'var(--color-bg-elevated)', padding: '0.75rem', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border-strong)' }}>
                      {otpauthUri}
                    </p>
                    <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: '0.5rem 0 0' }}>
                      {t('account.copyOtpauthUri')}
                    </p>
                  </div>
                ) : (
                  <div className="twofa-qr-code" aria-label={t('account.qrCodePlaceholder')} style={{ fontSize: '3rem' }}>⬡</div>
                )}
                <p style={{ fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', margin: 0, textAlign: 'center' }}>
                  {t('account.scanQRInstructions')}
                </p>
              </div>
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="twofa-code">{t('account.verificationCode')}</label>
                <input
                  id="twofa-code"
                  type="text"
                  className="settings-input"
                  placeholder={t('account.enter6DigitCode')}
                  maxLength={6}
                  pattern="[0-9]{6}"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={twoFaCode}
                  onChange={(e) => setTwoFaCode(e.target.value)}
                  style={{ fontFamily: 'var(--font-mono)', letterSpacing: '0.2em', maxWidth: 200 }}
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                <button
                  type="submit"
                  className="settings-save-btn"
                  style={{ margin: 0 }}
                  disabled={twoFaLoading}
                  aria-busy={twoFaLoading}
                >
                  {twoFaLoading ? (
                    <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('account.verifying')}</span></>
                  ) : t('account.verifyAndEnable')}
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => { setShowSetup2FA(false); setTwoFaCode(''); setTwoFaError(''); setOtpauthUri(''); setQrDataUrl(''); }}
                >
                  {t('common.cancel')}
                </button>
              </div>
            </form>
          ) : (
            <button
              className="settings-save-btn"
              onClick={handleBeginSetup2FA}
              disabled={twoFaLoading}
              style={{ margin: 0 }}
              aria-busy={twoFaLoading}
            >
              {twoFaLoading ? (
                <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('common.loading')}</span></>
              ) : t('account.setUp2FA')}
            </button>
          )}
        </section>

        {/* Sessions */}
        <section className="settings-section reveal reveal-4" aria-labelledby="sec-sessions-heading">
          <div className="settings-section-head">
            <div>
              <h2 id="sec-sessions-heading" className="settings-section-title">{t('account.activeSessions')}</h2>
              <p className="settings-section-desc" style={{ margin: 0 }}>
                {t('account.activeSessionsDesc')}
              </p>
            </div>
            {sessions.length > 1 && (
              <button className="settings-revoke-btn" onClick={handleSignOutAll} type="button">
                {t('account.signOutAll')}
              </button>
            )}
          </div>

          {sessionError && (
            <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {sessionError}
            </div>
          )}

          <div className="settings-sessions">
            {sessionsLoading ? (
              <div style={{ padding: '1rem', color: 'var(--color-text-muted)' }} aria-busy="true">
                <span className="spinner spinner-sm" aria-hidden="true" /> {t('account.loadingSessions')}
              </div>
            ) : sessions.length === 0 ? (
              <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>
                {t('account.noActiveSessions')}
              </p>
            ) : (
              sessions.map(session => (
                <div key={session.id} className="settings-session-item">
                  <div className="settings-session-icon" aria-hidden="true">
                    {session.current ? '◉' : '◎'}
                  </div>
                  <div className="settings-session-info">
                    <span className="settings-session-device">
                      {session.device}
                      {session.current && (
                        <span className="settings-session-badge">{t('account.currentSession')}</span>
                      )}
                    </span>
                    <span className="settings-session-meta">
                      {session.location} · {session.lastActive}
                    </span>
                  </div>
                  {!session.current && (
                    <button
                      type="button"
                      className="settings-revoke-btn"
                      onClick={() => handleRevokeSession(session.id)}
                      aria-label={t('account.revokeSessionLabel', { device: session.device })}
                    >
                      {t('account.revoke')}
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
        </section>

        {/* API Keys */}
        <section className="settings-section reveal reveal-5" aria-labelledby="sec-apikeys-heading">
          <div className="settings-section-head">
            <div>
              <h2 id="sec-apikeys-heading" className="settings-section-title">{t('account.apiKeys')}</h2>
              <p className="settings-section-desc" style={{ margin: 0 }}>
                {t('account.apiKeysDesc')}
              </p>
            </div>
            {!showNewKeyForm && (
              <button
                type="button"
                className="settings-save-btn"
                style={{ margin: 0 }}
                onClick={() => { setShowNewKeyForm(true); setCreatedKey(null); setApiKeysError(null); }}
              >
                {t('account.newKey')}
              </button>
            )}
          </div>

          {/* One-time new key display */}
          {createdKey && (
            <div
              role="status"
              aria-live="polite"
              style={{
                background: 'var(--color-accent-soft)',
                border: '1px solid var(--color-accent)',
                borderRadius: 'var(--radius)',
                padding: '1rem',
                marginBottom: '1rem',
              }}
            >
              <p style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)', color: 'var(--color-accent)', margin: '0 0 0.5rem' }}>
                {t('account.apiKeyCreated', { name: createdKey.name })}
              </p>
              <code
                style={{
                  display: 'block',
                  background: 'var(--color-bg-card)',
                  padding: '0.75rem',
                  borderRadius: 'var(--radius-sm)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: 'var(--text-sm)',
                  wordBreak: 'break-all',
                  color: 'var(--color-text-primary)',
                  userSelect: 'all',
                }}
              >
                {createdKey.key}
              </code>
              <button
                type="button"
                className="settings-action-btn"
                style={{ marginTop: '0.75rem' }}
                onClick={() => navigator.clipboard.writeText(createdKey.key).catch(() => null)}
              >
                {t('account.copyToClipboard')}
              </button>
              <button
                type="button"
                className="settings-action-btn"
                style={{ marginTop: '0.75rem', marginLeft: '0.5rem' }}
                onClick={() => setCreatedKey(null)}
              >
                {t('account.dismiss')}
              </button>
            </div>
          )}

          {apiKeysError && (
            <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {apiKeysError}
            </div>
          )}

          {showNewKeyForm && (
            <form
              className="apikey-new-display"
              onSubmit={handleCreateApiKey}
              style={{ marginBottom: '1rem' }}
              aria-label={t('account.newKeyFormLabel')}
            >
              <div className="settings-field" style={{ marginBottom: 0 }}>
                <label className="settings-field-label" htmlFor="key-name">{t('account.keyNameLabel')}</label>
                <input
                  id="key-name"
                  type="text"
                  className="settings-input"
                  placeholder={t('account.keyNamePlaceholder')}
                  value={newKeyName}
                  onChange={(e) => setNewKeyName(e.target.value)}
                  required
                  autoFocus
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem', marginTop: '0.75rem', flexWrap: 'wrap' }}>
                <button type="submit" className="settings-save-btn" style={{ margin: 0 }} disabled={creatingKey} aria-busy={creatingKey}>
                  {creatingKey ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('account.creatingKey')}</span></> : t('account.createKey')}
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => { setShowNewKeyForm(false); setNewKeyName(''); setApiKeysError(null); }}
                >
                  {t('common.cancel')}
                </button>
              </div>
            </form>
          )}

          {apiKeysLoading ? (
            <div style={{ padding: '1rem', color: 'var(--color-text-muted)' }} aria-busy="true">
              <span className="spinner spinner-sm" aria-hidden="true" /> {t('account.loadingApiKeys')}
            </div>
          ) : apiKeys.length === 0 ? (
            <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', padding: '0.5rem 0' }}>
              {t('account.noApiKeys')}
            </p>
          ) : (
            <div className="settings-sessions">
              {apiKeys.map(key => (
                <div key={key.id} className="settings-session-item">
                  <div className="settings-session-icon" aria-hidden="true">⌗</div>
                  <div className="settings-session-info">
                    <span className="settings-session-device">{key.name}</span>
                    <span className="settings-session-meta" style={{ fontFamily: 'var(--font-mono)' }}>
                      {key.prefix} · {t('account.apiKeyCreatedOn', { date: key.createdAt })} · {t('account.apiKeyLastUsed', { date: key.lastUsed })}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-revoke-btn"
                    onClick={() => handleRevokeApiKey(key.id)}
                    disabled={revokingId === key.id}
                    aria-label={t('account.revokeApiKeyLabel', { name: key.name })}
                  >
                    {revokingId === key.id ? t('account.revoking') : t('account.revoke')}
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </Layout>
  );
}
