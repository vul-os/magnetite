import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';

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
const MOCK_SESSIONS = import.meta.env.VITE_USE_MOCKS
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
    if (import.meta.env.VITE_USE_MOCKS) return;
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
    if (import.meta.env.VITE_USE_MOCKS) return;
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

  useEffect(() => { loadApiKeys(); }, [loadApiKeys]);

  const handlePasswordChange = async (e) => {
    e.preventDefault();
    setPasswordError('');
    setPasswordSuccess(false);

    if (passwords.new !== passwords.confirm) {
      setPasswordError('New passwords do not match');
      return;
    }
    if (passwords.new.length < 8) {
      setPasswordError('Password must be at least 8 characters');
      return;
    }
    if (!passwords.current) {
      setPasswordError('Current password is required');
      return;
    }

    setChangingPw(true);
    try {
      await api.auth.updatePassword(passwords.current, passwords.new);
      setPasswordSuccess(true);
      setPasswords({ current: '', new: '', confirm: '' });
      setTimeout(() => setPasswordSuccess(false), 3000);
    } catch (err) {
      setPasswordError(err.message || 'Failed to update password');
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
          <h1 className="settings-page-title">Security</h1>
          <p className="settings-page-subtitle">
            Manage your password, two-factor authentication, sessions, and API keys.
          </p>
        </header>

        {/* Password */}
        <section className="settings-section reveal reveal-2">
          <h2 className="settings-section-title">Change Password</h2>
          <p className="settings-section-desc">
            Use a strong password you don&apos;t use on other sites.
          </p>

          <form onSubmit={handlePasswordChange}>
            <div className="settings-field">
              <label className="settings-field-label" htmlFor="sec-cur-pw">Current Password</label>
              <input
                id="sec-cur-pw"
                type="password"
                value={passwords.current}
                onChange={(e) => setPasswords({ ...passwords, current: e.target.value })}
                className="settings-input"
                placeholder="Enter current password"
                autoComplete="current-password"
              />
            </div>
            <div className="settings-grid-2">
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="sec-new-pw">New Password</label>
                <input
                  id="sec-new-pw"
                  type="password"
                  value={passwords.new}
                  onChange={(e) => setPasswords({ ...passwords, new: e.target.value })}
                  className="settings-input"
                  placeholder="New password"
                  autoComplete="new-password"
                />
              </div>
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="sec-conf-pw">Confirm Password</label>
                <input
                  id="sec-conf-pw"
                  type="password"
                  value={passwords.confirm}
                  onChange={(e) => setPasswords({ ...passwords, confirm: e.target.value })}
                  className="settings-input"
                  placeholder="Confirm new password"
                  autoComplete="new-password"
                />
              </div>
            </div>

            {passwordError && (
              <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {passwordError}
              </div>
            )}
            {passwordSuccess && (
              <div className="auth-success" role="status" style={{ marginBottom: '1rem' }}>
                <span className="auth-success-icon" aria-hidden="true">✓</span>
                Password updated successfully!
              </div>
            )}

            <button type="submit" className="settings-save-btn" disabled={changingPw}>
              {changingPw ? (
                <><span className="spinner spinner-sm" aria-hidden="true" /> Updating&hellip;</>
              ) : 'Update Password'}
            </button>
          </form>
        </section>

        {/* 2FA */}
        <section className="settings-section reveal reveal-3">
          <h2 className="settings-section-title">Two-Factor Authentication</h2>
          <p className="settings-section-desc">
            Add a second layer of security with an authenticator app (Google Authenticator, Authy, etc.).
          </p>

          {twoFaError && (
            <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {twoFaError}
            </div>
          )}

          {twoFactorEnabled ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="twofa-status-enabled">
                <span aria-hidden="true">✓</span>
                2FA is enabled — your account is protected
              </div>
              {showDisableForm ? (
                <form onSubmit={handleDisable2FA} style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem', maxWidth: 320 }}>
                  <div className="settings-field">
                    <label className="settings-field-label" htmlFor="disable-2fa-code">
                      Enter your current 6-digit code to disable 2FA
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
                  <div style={{ display: 'flex', gap: '0.75rem' }}>
                    <button type="submit" className="settings-action-btn danger" disabled={twoFaLoading}>
                      {twoFaLoading ? <><span className="spinner spinner-sm" aria-hidden="true" /> Disabling&hellip;</> : 'Confirm Disable'}
                    </button>
                    <button type="button" className="settings-action-btn" onClick={() => { setShowDisableForm(false); setDisableCode(''); setTwoFaError(''); }}>
                      Cancel
                    </button>
                  </div>
                </form>
              ) : (
                <button
                  className="settings-action-btn"
                  onClick={() => setShowDisableForm(true)}
                  style={{ alignSelf: 'flex-start' }}
                >
                  Disable 2FA
                </button>
              )}
            </div>
          ) : showSetup2FA ? (
            <form onSubmit={handleVerify2FA} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="twofa-qr-placeholder">
                {qrDataUrl ? (
                  <img
                    src={qrDataUrl}
                    alt="Scan this QR code with your authenticator app"
                    style={{ width: 160, height: 160, imageRendering: 'pixelated' }}
                  />
                ) : otpauthUri ? (
                  <div style={{ maxWidth: 340 }}>
                    <p style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', wordBreak: 'break-all', color: 'var(--color-accent)', background: 'var(--color-bg-elevated)', padding: '0.75rem', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border-strong)' }}>
                      {otpauthUri}
                    </p>
                    <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', margin: '0.5rem 0 0' }}>
                      Copy this URI into your authenticator app if QR scanning is not available.
                    </p>
                  </div>
                ) : (
                  <div className="twofa-qr-code" aria-label="QR code placeholder" style={{ fontSize: '3rem' }}>⬡</div>
                )}
                <p style={{ fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', margin: 0, textAlign: 'center' }}>
                  Scan the QR code with your authenticator app, then enter the code below.
                </p>
              </div>
              <div className="settings-field">
                <label className="settings-field-label" htmlFor="twofa-code">Verification Code</label>
                <input
                  id="twofa-code"
                  type="text"
                  className="settings-input"
                  placeholder="Enter 6-digit code"
                  maxLength={6}
                  pattern="[0-9]{6}"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={twoFaCode}
                  onChange={(e) => setTwoFaCode(e.target.value)}
                  style={{ fontFamily: 'var(--font-mono)', letterSpacing: '0.2em', maxWidth: 200 }}
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <button
                  type="submit"
                  className="settings-save-btn"
                  style={{ margin: 0 }}
                  disabled={twoFaLoading}
                >
                  {twoFaLoading ? (
                    <><span className="spinner spinner-sm" aria-hidden="true" /> Verifying&hellip;</>
                  ) : 'Verify & Enable'}
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => { setShowSetup2FA(false); setTwoFaCode(''); setTwoFaError(''); setOtpauthUri(''); setQrDataUrl(''); }}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <button
              className="settings-save-btn"
              onClick={handleBeginSetup2FA}
              disabled={twoFaLoading}
              style={{ margin: 0 }}
            >
              {twoFaLoading ? (
                <><span className="spinner spinner-sm" aria-hidden="true" /> Loading&hellip;</>
              ) : 'Set Up 2FA'}
            </button>
          )}
        </section>

        {/* Sessions */}
        <section className="settings-section reveal reveal-4">
          <div className="settings-section-head">
            <div>
              <h2 className="settings-section-title">Active Sessions</h2>
              <p className="settings-section-desc" style={{ margin: 0 }}>
                Your account is signed in on these devices.
              </p>
            </div>
            {sessions.length > 1 && (
              <button className="settings-revoke-btn" onClick={handleSignOutAll}>
                Sign Out All
              </button>
            )}
          </div>

          {sessionError && (
            <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {sessionError}
            </div>
          )}

          <div className="settings-sessions">
            {sessionsLoading ? (
              <div style={{ padding: '1rem', color: 'var(--color-text-muted)' }} aria-busy="true">
                <span className="spinner spinner-sm" aria-hidden="true" /> Loading sessions&hellip;
              </div>
            ) : sessions.length === 0 ? (
              <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>
                No active sessions
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
                        <span className="settings-session-badge">Current</span>
                      )}
                    </span>
                    <span className="settings-session-meta">
                      {session.location} · {session.lastActive}
                    </span>
                  </div>
                  {!session.current && (
                    <button
                      className="settings-revoke-btn"
                      onClick={() => handleRevokeSession(session.id)}
                    >
                      Revoke
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
        </section>

        {/* API Keys */}
        <section className="settings-section reveal reveal-5">
          <div className="settings-section-head">
            <div>
              <h2 className="settings-section-title">API Keys</h2>
              <p className="settings-section-desc" style={{ margin: 0 }}>
                Programmatic access to Magnetite APIs.
              </p>
            </div>
            {!showNewKeyForm && (
              <button
                className="settings-save-btn"
                style={{ margin: 0 }}
                onClick={() => { setShowNewKeyForm(true); setCreatedKey(null); setApiKeysError(null); }}
              >
                + New Key
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
                ✓ API key &ldquo;{createdKey.name}&rdquo; created — copy it now, it will not be shown again:
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
                className="settings-action-btn"
                style={{ marginTop: '0.75rem' }}
                onClick={() => navigator.clipboard.writeText(createdKey.key).catch(() => null)}
              >
                Copy to Clipboard
              </button>
              <button
                className="settings-action-btn"
                style={{ marginTop: '0.75rem', marginLeft: '0.5rem' }}
                onClick={() => setCreatedKey(null)}
              >
                Dismiss
              </button>
            </div>
          )}

          {apiKeysError && (
            <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {apiKeysError}
            </div>
          )}

          {showNewKeyForm && (
            <form
              className="apikey-new-display"
              onSubmit={handleCreateApiKey}
              style={{ marginBottom: '1rem' }}
            >
              <div className="settings-field" style={{ marginBottom: 0 }}>
                <label className="settings-field-label" htmlFor="key-name">Key name</label>
                <input
                  id="key-name"
                  type="text"
                  className="settings-input"
                  placeholder="e.g., Production API Key"
                  value={newKeyName}
                  onChange={(e) => setNewKeyName(e.target.value)}
                  required
                  autoFocus
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem', marginTop: '0.75rem' }}>
                <button type="submit" className="settings-save-btn" style={{ margin: 0 }} disabled={creatingKey}>
                  {creatingKey ? <><span className="spinner spinner-sm" aria-hidden="true" /> Creating&hellip;</> : 'Create Key'}
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => { setShowNewKeyForm(false); setNewKeyName(''); setApiKeysError(null); }}
                >
                  Cancel
                </button>
              </div>
            </form>
          )}

          {apiKeysLoading ? (
            <div style={{ padding: '1rem', color: 'var(--color-text-muted)' }} aria-busy="true">
              <span className="spinner spinner-sm" aria-hidden="true" /> Loading API keys&hellip;
            </div>
          ) : apiKeys.length === 0 ? (
            <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', padding: '0.5rem 0' }}>
              No API keys yet. Create one to access the Magnetite API programmatically.
            </p>
          ) : (
            <div className="settings-sessions">
              {apiKeys.map(key => (
                <div key={key.id} className="settings-session-item">
                  <div className="settings-session-icon" aria-hidden="true">⌗</div>
                  <div className="settings-session-info">
                    <span className="settings-session-device">{key.name}</span>
                    <span className="settings-session-meta" style={{ fontFamily: 'var(--font-mono)' }}>
                      {key.prefix} · Created {key.createdAt} · Last used: {key.lastUsed}
                    </span>
                  </div>
                  <button
                    className="settings-revoke-btn"
                    onClick={() => handleRevokeApiKey(key.id)}
                    disabled={revokingId === key.id}
                    aria-label={`Revoke API key ${key.name}`}
                  >
                    {revokingId === key.id ? 'Revoking…' : 'Revoke'}
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
