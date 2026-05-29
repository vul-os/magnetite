import { useState } from 'react';
import Layout from '../components/Layout';

const MOCK_SESSIONS = [
  { id: 'sess_001', device: 'Chrome on Mac OS', location: 'San Francisco, CA', lastActive: 'Now', current: true },
  { id: 'sess_002', device: 'Safari on iPhone', location: 'San Francisco, CA', lastActive: '2 hours ago', current: false },
  { id: 'sess_003', device: 'Firefox on Windows', location: 'New York, NY', lastActive: 'Yesterday', current: false },
];

const MOCK_API_KEYS = [
  { id: 'key_001', name: 'Production API Key', key: 'mgn_live_***************xyz', created: '2026-03-15', lastUsed: '2026-05-18' },
  { id: 'key_002', name: 'Development API Key', key: 'mgn_test_***************abc', created: '2026-04-20', lastUsed: '2026-05-17' },
];

export default function Security() {
  const [passwords, setPasswords]       = useState({ current: '', new: '', confirm: '' });
  const [passwordError, setPasswordError]   = useState('');
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [showSetup2FA, setShowSetup2FA]   = useState(false);
  const [sessions, setSessions]           = useState(MOCK_SESSIONS);
  const [apiKeys, setApiKeys]             = useState(MOCK_API_KEYS);
  const [showNewKeyForm, setShowNewKeyForm] = useState(false);
  const [newKeyName, setNewKeyName]       = useState('');
  const [newApiKey, setNewApiKey]         = useState(null);

  const handlePasswordChange = (e) => {
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

    setPasswordSuccess(true);
    setPasswords({ current: '', new: '', confirm: '' });
    setTimeout(() => setPasswordSuccess(false), 3000);
  };

  const handleVerify2FA = (e) => {
    e.preventDefault();
    setTwoFactorEnabled(true);
    setShowSetup2FA(false);
  };

  const handleSignOutAll = () => {
    setSessions(prev => prev.filter(s => s.current));
  };

  const handleRevokeSession = (sessionId) => {
    setSessions(prev => prev.filter(s => s.id !== sessionId));
  };

  const handleCreateApiKey = (e) => {
    e.preventDefault();
    if (!newKeyName.trim()) return;

    const mockKey = {
      id: `key_${Date.now()}`,
      name: newKeyName,
      key: `mgn_live_${Math.random().toString(36).substring(2, 15)}`,
      created: new Date().toISOString().split('T')[0],
      lastUsed: 'Never',
    };

    setApiKeys(prev => [...prev, mockKey]);
    setNewApiKey(mockKey.key);
    setNewKeyName('');
    setShowNewKeyForm(false);
  };

  const handleDeleteApiKey = (keyId) => {
    setApiKeys(prev => prev.filter(k => k.id !== keyId));
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

            <button type="submit" className="settings-save-btn">Update Password</button>
          </form>
        </section>

        {/* 2FA */}
        <section className="settings-section reveal reveal-3">
          <h2 className="settings-section-title">Two-Factor Authentication</h2>
          <p className="settings-section-desc">
            Add a second layer of security with an authenticator app.
          </p>

          {twoFactorEnabled ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="twofa-status-enabled">
                <span aria-hidden="true">✓</span>
                2FA is enabled — your account is protected
              </div>
              <button className="settings-action-btn" onClick={() => setTwoFactorEnabled(false)} style={{ alignSelf: 'flex-start' }}>
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
                  style={{ fontFamily: 'var(--font-mono)', letterSpacing: '0.2em', maxWidth: 200 }}
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <button type="submit" className="settings-save-btn" style={{ margin: 0 }}>Verify &amp; Enable</button>
                <button type="button" className="settings-action-btn" onClick={() => setShowSetup2FA(false)}>Cancel</button>
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
          <div className="settings-sessions">
            {sessions.map(session => (
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
            ))}
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
            <button
              className="settings-save-btn"
              style={{ margin: 0 }}
              onClick={() => setShowNewKeyForm(true)}
            >
              + New Key
            </button>
          </div>

          {showNewKeyForm && (
            <form className="apikey-new-display" onSubmit={handleCreateApiKey} style={{ marginBottom: '1rem' }}>
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

          {newApiKey && (
            <div className="apikey-new-display" style={{ marginBottom: '1rem' }}>
              <p className="apikey-warning">
                ⚠ Copy this key now — you won&apos;t see it again.
              </p>
              <div className="apikey-value-row">
                <code className="apikey-code">{newApiKey}</code>
                <button
                  className="settings-action-btn"
                  onClick={() => navigator.clipboard.writeText(newApiKey)}
                >
                  Copy
                </button>
              </div>
              <button className="settings-save-btn" style={{ margin: 0 }} onClick={() => setNewApiKey(null)}>
                Done
              </button>
            </div>
          )}

          <div className="apikeys-list">
            {apiKeys.map(key => (
              <div key={key.id} className="apikey-item">
                <div className="apikey-item-info">
                  <span className="apikey-item-name">{key.name}</span>
                  <span className="apikey-item-value">{key.key}</span>
                  <div className="apikey-item-meta">
                    <span>Created {key.created}</span>
                    <span>Last used {key.lastUsed}</span>
                  </div>
                </div>
                <button
                  className="settings-revoke-btn"
                  onClick={() => handleDeleteApiKey(key.id)}
                >
                  Delete
                </button>
              </div>
            ))}
          </div>
        </section>
      </div>
    </Layout>
  );
}
