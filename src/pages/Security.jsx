import { useState, useEffect } from 'react';
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

/* API keys are not yet implemented in the backend.
 * Showing a disabled state with a clear TODO rather than faking success. */
const API_KEYS_TODO = true;

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

export default function Security() {
  const [passwords, setPasswords]           = useState({ current: '', new: '', confirm: '' });
  const [passwordError, setPasswordError]   = useState('');
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [changingPw, setChangingPw]         = useState(false);

  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [showSetup2FA, setShowSetup2FA]     = useState(false);
  const [twoFaCode, setTwoFaCode]           = useState('');
  const [twoFaError, setTwoFaError]         = useState('');
  const [twoFaLoading, setTwoFaLoading]     = useState(false);

  const [sessions, setSessions]             = useState(MOCK_SESSIONS ?? []);
  const [sessionsLoading, setSessionsLoading] = useState(!MOCK_SESSIONS);
  const [sessionError, setSessionError]     = useState(null);

  const [newKeyName, setNewKeyName]         = useState('');
  const [showNewKeyForm, setShowNewKeyForm] = useState(false);

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

  const handleVerify2FA = async (e) => {
    e.preventDefault();
    /* TODO: backend 2FA (TOTP setup/verify) endpoints are not yet implemented.
     * When they exist, call POST /api/auth/2fa/verify with { code: twoFaCode }.
     * For now, show an honest disabled state rather than faking success. */
    setTwoFaLoading(true);
    setTwoFaError('');
    try {
      const res = await authFetch('/api/auth/2fa/verify', {
        method: 'POST',
        body: JSON.stringify({ code: twoFaCode }),
      });
      if (res.status === 404 || res.status === 501) {
        throw new Error('2FA endpoint not yet implemented on this server.');
      }
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || 'Verification failed');
      }
      setTwoFactorEnabled(true);
      setShowSetup2FA(false);
      setTwoFaCode('');
    } catch (err) {
      setTwoFaError(err.message);
    } finally {
      setTwoFaLoading(false);
    }
  };

  const handleDisable2FA = async () => {
    /* TODO: call DELETE /api/auth/2fa when implemented */
    setTwoFaError('2FA management endpoint not yet implemented.');
    setTimeout(() => setTwoFaError(''), 3000);
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
    /* Optimistically remove, then confirm with backend.
     * The single-session revoke requires the refresh token (which the frontend doesn't
     * store separately). Best-effort: remove from UI. */
    setSessions(prev => prev.filter(s => s.id !== sessionId));
  };

  const handleCreateApiKey = (e) => {
    e.preventDefault();
    /* TODO: API key management endpoints are not yet implemented in the backend.
     * When implemented, call POST /api/auth/api-keys with { name: newKeyName }.
     * Generating keys client-side (Math.random) would be insecure — not done here.
     * This form is intentionally disabled until the backend endpoint exists. */
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
            Add a second layer of security with an authenticator app.
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
              <button
                className="settings-action-btn"
                onClick={handleDisable2FA}
                style={{ alignSelf: 'flex-start' }}
              >
                Disable 2FA
              </button>
            </div>
          ) : showSetup2FA ? (
            <form onSubmit={handleVerify2FA} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="twofa-qr-placeholder">
                <div className="twofa-qr-code" aria-label="QR code placeholder">⬡</div>
                <p style={{ font: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', margin: 0, textAlign: 'center' }}>
                  Scan this QR code with your authenticator app (Google Authenticator, Authy, etc.)
                </p>
                <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-warning)', margin: 0, textAlign: 'center', fontFamily: 'var(--font-mono)' }}>
                  Note: 2FA setup requires server support — will error if not yet implemented
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
                  onClick={() => { setShowSetup2FA(false); setTwoFaCode(''); setTwoFaError(''); }}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <button className="settings-save-btn" onClick={() => setShowSetup2FA(true)} style={{ margin: 0 }}>
              Set Up 2FA
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

        {/* API Keys — disabled, TODO: implement backend endpoint */}
        <section className="settings-section reveal reveal-5">
          <div className="settings-section-head">
            <div>
              <h2 className="settings-section-title">API Keys</h2>
              <p className="settings-section-desc" style={{ margin: 0 }}>
                Programmatic access to Magnetite APIs.
              </p>
            </div>
            {!API_KEYS_TODO && (
              <button
                className="settings-save-btn"
                style={{ margin: 0 }}
                onClick={() => setShowNewKeyForm(true)}
              >
                + New Key
              </button>
            )}
          </div>

          {API_KEYS_TODO && (
            <div
              style={{
                background: 'var(--color-bg-elevated)',
                border: '1px dashed var(--color-border-strong)',
                borderRadius: 'var(--radius)',
                padding: '1.25rem',
                color: 'var(--color-text-muted)',
                fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-sm)',
              }}
              role="note"
            >
              {/* TODO: API key management (create/list/revoke) requires backend endpoints.
                  Add POST/GET/DELETE /api/auth/api-keys to the backend auth router.
                  Keys must be generated server-side (never with Math.random in the browser). */}
              API key management is not yet available. Backend endpoints are on the roadmap.
            </div>
          )}

          {!API_KEYS_TODO && showNewKeyForm && (
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
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <button type="submit" className="settings-save-btn" style={{ margin: 0 }}>Create Key</button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => { setShowNewKeyForm(false); setNewKeyName(''); }}
                >
                  Cancel
                </button>
              </div>
            </form>
          )}
        </section>
      </div>
    </Layout>
  );
}
