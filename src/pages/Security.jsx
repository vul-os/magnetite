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
  const [passwords, setPasswords] = useState({ current: '', new: '', confirm: '' });
  const [passwordError, setPasswordError] = useState('');
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [showSetup2FA, setShowSetup2FA] = useState(false);
  const [sessions, setSessions] = useState(MOCK_SESSIONS);
  const [apiKeys, setApiKeys] = useState(MOCK_API_KEYS);
  const [showNewKeyForm, setShowNewKeyForm] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [newApiKey, setNewApiKey] = useState(null);

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

  const handleSetup2FA = () => {
    setShowSetup2FA(true);
  };

  const handleVerify2FA = (e) => {
    e.preventDefault();
    setTwoFactorEnabled(true);
    setShowSetup2FA(false);
  };

  const handleDisable2FA = () => {
    setTwoFactorEnabled(false);
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
      <div className="settings-page security-page">
        <header className="settings-header">
          <h1>Security</h1>
          <p>Manage your password, two-factor authentication, and active sessions</p>
        </header>

        <div className="settings-content">
          <div className="form-section">
            <h3>Change Password</h3>
            <form className="settings-form" onSubmit={handlePasswordChange}>
              <div className="form-group">
                <label>Current Password</label>
                <input
                  type="password"
                  value={passwords.current}
                  onChange={(e) => setPasswords({ ...passwords, current: e.target.value })}
                  placeholder="Enter current password"
                />
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>New Password</label>
                  <input
                    type="password"
                    value={passwords.new}
                    onChange={(e) => setPasswords({ ...passwords, new: e.target.value })}
                    placeholder="Enter new password"
                  />
                </div>
                <div className="form-group">
                  <label>Confirm New Password</label>
                  <input
                    type="password"
                    value={passwords.confirm}
                    onChange={(e) => setPasswords({ ...passwords, confirm: e.target.value })}
                    placeholder="Confirm new password"
                  />
                </div>
              </div>
              {passwordError && <p className="error-message">{passwordError}</p>}
              {passwordSuccess && <p className="success-message">Password updated successfully!</p>}
              <button type="submit" className="btn btn-primary">Update Password</button>
            </form>
          </div>

          <div className="form-section">
            <h3>Two-Factor Authentication</h3>
            {twoFactorEnabled ? (
              <div className="twofa-enabled">
                <div className="status-badge enabled">
                  <span className="status-icon">✓</span>
                  2FA is enabled
                </div>
                <p>Your account is protected with two-factor authentication.</p>
                <button className="btn btn-secondary" onClick={handleDisable2FA}>
                  Disable 2FA
                </button>
              </div>
            ) : showSetup2FA ? (
              <form className="setup-2fa-form" onSubmit={handleVerify2FA}>
                <div className="qr-placeholder">
                  <div className="qr-code">
                    <span className="qr-icon">⬡</span>
                  </div>
                  <p>Scan this QR code with your authenticator app</p>
                </div>
                <div className="form-group">
                  <label>Verification Code</label>
                  <input
                    type="text"
                    placeholder="Enter 6-digit code"
                    maxLength={6}
                    pattern="[0-9]{6}"
                  />
                </div>
                <div className="form-actions">
                  <button type="submit" className="btn btn-primary">Verify & Enable</button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowSetup2FA(false)}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            ) : (
              <div className="twofa-disabled">
                <p>Add an extra layer of security to your account by enabling two-factor authentication.</p>
                <button className="btn btn-primary" onClick={handleSetup2FA}>
                  Set Up 2FA
                </button>
              </div>
            )}
          </div>

          <div className="form-section">
            <div className="section-header">
              <div>
                <h3>Active Sessions</h3>
                <p className="section-description">Manage your active sessions across devices</p>
              </div>
              {sessions.length > 1 && (
                <button
                  className="btn btn-secondary danger"
                  onClick={handleSignOutAll}
                >
                  Sign Out All Devices
                </button>
              )}
            </div>
            <div className="sessions-list">
              {sessions.map(session => (
                <div key={session.id} className="session-item">
                  <div className="session-info">
                    <span className="session-device">{session.device}</span>
                    <span className="session-meta">
                      {session.location} · {session.lastActive}
                      {session.current && <span className="current-badge">Current</span>}
                    </span>
                  </div>
                  {!session.current && (
                    <button
                      className="btn btn-secondary"
                      onClick={() => handleRevokeSession(session.id)}
                    >
                      Revoke
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>

          <div className="form-section">
            <div className="section-header">
              <div>
                <h3>API Keys</h3>
                <p className="section-description">Manage your API keys for programmatic access</p>
              </div>
              <button
                className="btn btn-primary"
                onClick={() => setShowNewKeyForm(true)}
              >
                + Create New Key
              </button>
            </div>

            {showNewKeyForm && (
              <form className="new-key-form" onSubmit={handleCreateApiKey}>
                <div className="form-group">
                  <label>Key Name</label>
                  <input
                    type="text"
                    placeholder="e.g., Production API Key"
                    value={newKeyName}
                    onChange={(e) => setNewKeyName(e.target.value)}
                    required
                  />
                </div>
                <div className="form-actions">
                  <button type="submit" className="btn btn-primary">Create Key</button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => { setShowNewKeyForm(false); setNewKeyName(''); }}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            )}

            {newApiKey && (
              <div className="new-key-display">
                <h4>New API Key Created</h4>
                <p className="key-warning">Copy this key now. You won't be able to see it again.</p>
                <div className="key-value">
                  <code>{newApiKey}</code>
                  <button
                    className="btn btn-secondary"
                    onClick={() => navigator.clipboard.writeText(newApiKey)}
                  >
                    Copy
                  </button>
                </div>
                <button
                  className="btn btn-primary"
                  onClick={() => setNewApiKey(null)}
                >
                  Done
                </button>
              </div>
            )}

            <div className="api-keys-list">
              {apiKeys.map(key => (
                <div key={key.id} className="api-key-item">
                  <div className="key-info">
                    <span className="key-name">{key.name}</span>
                    <code className="key-value-display">{key.key}</code>
                    <div className="key-meta">
                      <span>Created: {key.created}</span>
                      <span>Last used: {key.lastUsed}</span>
                    </div>
                  </div>
                  <button
                    className="btn btn-secondary"
                    onClick={() => handleDeleteApiKey(key.id)}
                  >
                    Delete
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </Layout>
  );
}