import { useState } from 'react';
import Layout from '../components/Layout';

const MOCK_BLOCKED_USERS = [
  { id: 'user_001', username: 'cheater123', blockedDate: '2026-04-10' },
  { id: 'user_002', username: 'toxic_player', blockedDate: '2026-05-02' },
];

function ToggleSetting({ label, description, checked, onChange }) {
  return (
    <div className="settings-toggle-row">
      <div className="settings-toggle-text">
        <span className="settings-toggle-label">{label}</span>
        {description && <p className="settings-toggle-desc">{description}</p>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className={`settings-toggle-switch${checked ? ' checked' : ''}`}
        onClick={() => onChange(!checked)}
      >
        <span className="settings-toggle-thumb" />
      </button>
    </div>
  );
}

export default function PrivacySettings() {
  const [privacy, setPrivacy] = useState({
    profileVisibility:  'public',
    showOnLeaderboards: true,
    showOnlineStatus:   true,
    allowFriendRequests: true,
  });
  const [blockedUsers, setBlockedUsers] = useState(MOCK_BLOCKED_USERS);
  const [saving, setSaving]             = useState(false);
  const [saveSuccess, setSaveSuccess]   = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [exporting, setExporting]       = useState(false);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 800));
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2500);
    } catch { /* noop */ }
    finally { setSaving(false); }
  };

  const handleUnblockUser = (userId) => {
    setBlockedUsers(prev => prev.filter(u => u.id !== userId));
  };

  const handleExportData = async () => {
    setExporting(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 2000));
    } catch { /* noop */ }
    finally { setExporting(false); }
  };

  const handleDeleteAccount = () => {
    setShowDeleteConfirm(false);
    /* In production: call api.auth.deleteAccount() */
  };

  return (
    <Layout>
      <div className="security-page">
        {/* Header */}
        <header className="settings-page-header reveal reveal-1">
          <span className="kicker">// PRIVACY</span>
          <h1 className="settings-page-title">Privacy</h1>
          <p className="settings-page-subtitle">
            Control who can see your data and how your information is used.
          </p>
        </header>

        <form onSubmit={handleSave}>
          {/* Profile Visibility */}
          <section className="settings-section reveal reveal-2">
            <h2 className="settings-section-title">Profile Visibility</h2>
            <div className="settings-field" style={{ maxWidth: 340, marginBottom: 0 }}>
              <label className="settings-field-label" htmlFor="priv-vis">Who can view your profile</label>
              <select
                id="priv-vis"
                value={privacy.profileVisibility}
                onChange={(e) => setPrivacy({ ...privacy, profileVisibility: e.target.value })}
                className="settings-input"
              >
                <option value="public">Public — anyone can view</option>
                <option value="friends">Friends Only</option>
                <option value="private">Private — only you</option>
              </select>
            </div>
          </section>

          {/* Visibility options */}
          <section className="settings-section reveal reveal-3">
            <h2 className="settings-section-title">Visibility Options</h2>
            <ToggleSetting
              label="Show on Leaderboards"
              description="Appear in public rankings with your scores and stats."
              checked={privacy.showOnLeaderboards}
              onChange={(v) => setPrivacy({ ...privacy, showOnLeaderboards: v })}
            />
            <ToggleSetting
              label="Show Online Status"
              description="Let other players see when you&apos;re active."
              checked={privacy.showOnlineStatus}
              onChange={(v) => setPrivacy({ ...privacy, showOnlineStatus: v })}
            />
            <ToggleSetting
              label="Allow Friend Requests"
              description="Other players can send you friend requests."
              checked={privacy.allowFriendRequests}
              onChange={(v) => setPrivacy({ ...privacy, allowFriendRequests: v })}
            />
          </section>

          {/* Blocked users */}
          <section className="settings-section reveal reveal-4">
            <h2 className="settings-section-title">Blocked Users</h2>
            <p className="settings-section-desc">
              Blocked players cannot view your profile or send you messages.
            </p>
            {blockedUsers.length === 0 ? (
              <p style={{ fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-muted)', margin: 0 }}>
                You haven&apos;t blocked any users.
              </p>
            ) : (
              <div className="settings-blocked-list">
                {blockedUsers.map(user => (
                  <div key={user.id} className="settings-blocked-item">
                    <div>
                      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-accent)' }}>
                        @{user.username}
                      </span>
                      <span style={{ marginLeft: '0.75rem', fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--color-text-muted)' }}>
                        Blocked {user.blockedDate}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="settings-action-btn"
                      onClick={() => handleUnblockUser(user.id)}
                    >
                      Unblock
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>

          <button type="submit" className="settings-save-btn reveal reveal-5" disabled={saving} style={{ marginBottom: '2rem' }}>
            {saving
              ? <><span className="spinner spinner-sm" aria-hidden="true" /> Saving&hellip;</>
              : saveSuccess
                ? <><span style={{ color: 'var(--color-success)' }}>✓</span> Saved!</>
                : 'Save Changes'}
          </button>
        </form>

        {/* Data Management */}
        <section className="settings-section reveal reveal-6">
          <h2 className="settings-section-title">Data Management</h2>
          <div className="settings-danger-row">
            <div>
              <span className="settings-danger-action-title">Export Your Data</span>
              <p className="settings-danger-desc">
                Download a copy of all your profile, game, and transaction data.
              </p>
            </div>
            <button
              type="button"
              className="settings-action-btn"
              onClick={handleExportData}
              disabled={exporting}
            >
              {exporting
                ? <><span className="spinner spinner-sm" aria-hidden="true" /> Exporting&hellip;</>
                : 'Request Export'}
            </button>
          </div>
        </section>

        {/* Danger zone */}
        <section className="settings-section settings-danger-zone reveal reveal-7">
          <h2 className="settings-section-title">Danger Zone</h2>
          <div className="settings-danger-row">
            <div>
              <span className="settings-danger-action-title">Delete Account</span>
              <p className="settings-danger-desc">
                Permanently delete your account and all associated data. This cannot be undone.
              </p>
            </div>
            {showDeleteConfirm ? (
              <div style={{ display: 'flex', gap: '0.625rem', flexWrap: 'wrap' }}>
                <button
                  type="button"
                  className="settings-revoke-btn"
                  style={{ padding: '0.5rem 1rem', border: '1px solid var(--color-error)', background: 'rgba(255,84,104,0.1)', color: 'var(--color-error)' }}
                  onClick={handleDeleteAccount}
                >
                  Yes, delete
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => setShowDeleteConfirm(false)}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                type="button"
                className="settings-revoke-btn"
                style={{ padding: '0.5rem 1rem', border: '1px solid rgba(255,84,104,0.4)', color: 'var(--color-error)' }}
                onClick={() => setShowDeleteConfirm(true)}
              >
                Delete Account
              </button>
            )}
          </div>
        </section>
      </div>
    </Layout>
  );
}
