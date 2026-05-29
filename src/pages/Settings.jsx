import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import ThemeToggle from '../components/ThemeToggle';
import { api } from '../api/client';

const MOCK_USER = {
  name: 'StarForge Studios',
  email: 'dev@starforge.com',
  avatar: 'https://picsum.photos/seed/avatar/200/200',
  bio: 'Indie game developer specialising in action and RPG games built in Rust.',
  location: 'San Francisco, CA',
};

const MOCK_SESSIONS = [
  { id: 'sess_001', device: 'Chrome on Mac OS', location: 'San Francisco, CA', lastActive: 'Now', current: true },
  { id: 'sess_002', device: 'Safari on iPhone', location: 'San Francisco, CA', lastActive: '2 hours ago', current: false },
  { id: 'sess_003', device: 'Firefox on Windows', location: 'New York, NY', lastActive: 'Yesterday', current: false },
];

/* ── Reusable Toggle component ─────────────────────────────────────────────── */
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

/* ── SaveButton ─────────────────────────────────────────────────────────────── */
function SaveButton({ saving, success }) {
  return (
    <button type="submit" className="settings-save-btn" disabled={saving}>
      {saving
        ? <><span className="spinner spinner-sm" aria-hidden="true" /> Saving&hellip;</>
        : success
          ? <><span style={{ color: 'var(--color-success)' }}>✓</span> Saved!</>
          : 'Save Changes'}
    </button>
  );
}

/* ── Tab definitions ─────────────────────────────────────────────────────────── */
const TABS = [
  { id: 'profile',       label: 'Profile',       icon: '◉' },
  { id: 'account',       label: 'Account',        icon: '⬡' },
  { id: 'appearance',    label: 'Appearance',     icon: '◈' },
  { id: 'notifications', label: 'Notifications',  icon: '◎' },
  { id: 'privacy',       label: 'Privacy',        icon: '⊕' },
];

export default function Settings() {
  const [activeTab, setActiveTab] = useState('profile');
  const [profile, setProfile]     = useState(MOCK_USER);
  const [account, setAccount]     = useState({ email: MOCK_USER.email, twoFactorEnabled: false });
  const [notifications, setNotifications] = useState({
    email: { promotions: true, updates: true, newsletter: false },
    push:  { matches: true, friends: true, system: false },
    frequency: 'instant',
  });
  const [privacy, setPrivacy] = useState({
    profileVisibility: 'public',
    showOnLeaderboards: true,
    blockedUsers: [],
  });
  const [saving, setSaving]       = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [sessions, setSessions]   = useState(MOCK_SESSIONS);
  const [loading, setLoading]     = useState(true);

  useEffect(() => {
    async function loadData() {
      try {
        const userData = await api.auth.me();
        if (userData?.user) {
          setProfile(prev => ({ ...prev, ...userData.user }));
        }
      } catch {
        /* use mock */
      } finally {
        setLoading(false);
      }
    }
    loadData();
  }, []);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 800));
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2500);
    } catch {
      /* noop */
    } finally {
      setSaving(false);
    }
  };

  const handleAvatarUpload = (e) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onloadend = () => setProfile(prev => ({ ...prev, avatar: reader.result }));
    reader.readAsDataURL(file);
  };

  const handleRevokeSession = (sessionId) => {
    setSessions(prev => prev.filter(s => s.id !== sessionId));
  };

  /* ── Tab panels ─────────────────────────────────────────────────────────── */

  const renderProfile = () => (
    <form className="settings-tab-panel" onSubmit={handleSave}>
      <div className="settings-section">
        <h3 className="settings-section-title">Profile Information</h3>
        <p className="settings-section-desc">Visible to other players and developers on Magnetite.</p>

        {/* Avatar */}
        <div className="settings-avatar-row">
          <div className="settings-avatar-wrap">
            <img src={profile.avatar} alt="Your avatar" className="settings-avatar" loading="lazy" />
            <div className="settings-avatar-overlay" aria-hidden="true">
              <span>Change</span>
            </div>
            <label className="settings-avatar-input-label" aria-label="Upload new avatar">
              <input type="file" accept="image/*" onChange={handleAvatarUpload} style={{ display: 'none' }} />
            </label>
          </div>
          <div className="settings-avatar-meta">
            <span className="settings-field-label">AVATAR</span>
            <p className="settings-avatar-hint">Recommended 200×200 px · JPG or PNG · Max 2 MB</p>
          </div>
        </div>

        <div className="settings-grid-2">
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="set-username">Username</label>
            <input
              id="set-username"
              type="text"
              value={profile.name}
              onChange={(e) => setProfile({ ...profile, name: e.target.value })}
              className="settings-input"
            />
          </div>
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="set-location">Location</label>
            <input
              id="set-location"
              type="text"
              value={profile.location}
              onChange={(e) => setProfile({ ...profile, location: e.target.value })}
              className="settings-input"
              placeholder="City, Country"
            />
          </div>
        </div>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="set-bio">Bio</label>
          <textarea
            id="set-bio"
            value={profile.bio}
            onChange={(e) => setProfile({ ...profile, bio: e.target.value })}
            className="settings-input settings-textarea"
            rows={3}
            placeholder="Tell the community about yourself and your games…"
          />
        </div>
      </div>
      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  const renderAccount = () => (
    <form className="settings-tab-panel" onSubmit={handleSave}>
      <div className="settings-section">
        <h3 className="settings-section-title">Email Address</h3>
        <div className="settings-input-action">
          <input
            type="email"
            value={account.email}
            onChange={(e) => setAccount({ ...account, email: e.target.value })}
            className="settings-input"
            aria-label="Email address"
          />
          <button type="button" className="settings-action-btn">Change Email</button>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">Password</h3>
        <div className="settings-grid-2">
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="cur-pw-set">Current Password</label>
            <input id="cur-pw-set" type="password" className="settings-input" placeholder="Enter current password" />
          </div>
          <div />
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="new-pw-set">New Password</label>
            <input id="new-pw-set" type="password" className="settings-input" placeholder="New password" />
          </div>
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="conf-pw-set">Confirm New Password</label>
            <input id="conf-pw-set" type="password" className="settings-input" placeholder="Confirm new password" />
          </div>
        </div>
        <button type="button" className="settings-action-btn">Update Password</button>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">Two-Factor Authentication</h3>
        <ToggleSetting
          label="Enable 2FA"
          description="Add a second layer of security with an authenticator app."
          checked={account.twoFactorEnabled}
          onChange={(v) => setAccount({ ...account, twoFactorEnabled: v })}
        />
      </div>

      <div className="settings-section">
        <div className="settings-section-head">
          <div>
            <h3 className="settings-section-title">Active Sessions</h3>
            <p className="settings-section-desc">Your account is signed in on these devices.</p>
          </div>
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
                  type="button"
                  className="settings-revoke-btn"
                  onClick={() => handleRevokeSession(session.id)}
                >
                  Revoke
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  const renderAppearance = () => (
    <div className="settings-tab-panel">
      <div className="settings-section">
        <h3 className="settings-section-title">Theme</h3>
        <p className="settings-section-desc">Choose your preferred colour scheme.</p>
        <div className="settings-theme-row">
          <ThemeToggle />
        </div>
      </div>
    </div>
  );

  const renderNotifications = () => (
    <form className="settings-tab-panel" onSubmit={handleSave}>
      <div className="settings-section">
        <h3 className="settings-section-title">Email Notifications</h3>
        <ToggleSetting
          label="Promotional Emails"
          description="Updates about new games and special offers."
          checked={notifications.email.promotions}
          onChange={(v) => setNotifications({ ...notifications, email: { ...notifications.email, promotions: v } })}
        />
        <ToggleSetting
          label="Account Updates"
          description="Important account and security notifications."
          checked={notifications.email.updates}
          onChange={(v) => setNotifications({ ...notifications, email: { ...notifications.email, updates: v } })}
        />
        <ToggleSetting
          label="Newsletter"
          description="Weekly community digest and platform news."
          checked={notifications.email.newsletter}
          onChange={(v) => setNotifications({ ...notifications, email: { ...notifications.email, newsletter: v } })}
        />
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">Push Notifications</h3>
        <ToggleSetting
          label="Match Found"
          description="When matchmaking finds a game for you."
          checked={notifications.push.matches}
          onChange={(v) => setNotifications({ ...notifications, push: { ...notifications.push, matches: v } })}
        />
        <ToggleSetting
          label="Friend Activity"
          description="When friends join, leave, or come online."
          checked={notifications.push.friends}
          onChange={(v) => setNotifications({ ...notifications, push: { ...notifications.push, friends: v } })}
        />
        <ToggleSetting
          label="System Announcements"
          description="Platform maintenance and system messages."
          checked={notifications.push.system}
          onChange={(v) => setNotifications({ ...notifications, push: { ...notifications.push, system: v } })}
        />
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">Email Frequency</h3>
        <div className="settings-field" style={{ maxWidth: 260 }}>
          <label className="settings-field-label" htmlFor="notif-freq">Digest frequency</label>
          <select
            id="notif-freq"
            value={notifications.frequency}
            onChange={(e) => setNotifications({ ...notifications, frequency: e.target.value })}
            className="settings-input"
          >
            <option value="instant">Instant</option>
            <option value="hourly">Hourly Digest</option>
            <option value="daily">Daily Digest</option>
            <option value="weekly">Weekly Digest</option>
            <option value="never">Never</option>
          </select>
        </div>
      </div>

      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  const renderPrivacy = () => (
    <form className="settings-tab-panel" onSubmit={handleSave}>
      <div className="settings-section">
        <h3 className="settings-section-title">Profile Visibility</h3>
        <div className="settings-field" style={{ maxWidth: 320 }}>
          <label className="settings-field-label" htmlFor="prof-vis">Who can view your profile</label>
          <select
            id="prof-vis"
            value={privacy.profileVisibility}
            onChange={(e) => setPrivacy({ ...privacy, profileVisibility: e.target.value })}
            className="settings-input"
          >
            <option value="public">Public — anyone can view</option>
            <option value="friends">Friends Only</option>
            <option value="private">Private — only you</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">Visibility Options</h3>
        <ToggleSetting
          label="Show on Leaderboards"
          description="Appear in public rankings with your scores."
          checked={privacy.showOnLeaderboards}
          onChange={(v) => setPrivacy({ ...privacy, showOnLeaderboards: v })}
        />
      </div>

      {privacy.blockedUsers.length > 0 && (
        <div className="settings-section">
          <h3 className="settings-section-title">Blocked Users</h3>
          <p className="settings-section-desc">Blocked users cannot view your profile or message you.</p>
          <div className="settings-blocked-list">
            {privacy.blockedUsers.map(userId => (
              <div key={userId} className="settings-blocked-item">
                <span>Blocked User #{userId}</span>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => setPrivacy(prev => ({ ...prev, blockedUsers: prev.blockedUsers.filter(id => id !== userId) }))}
                >
                  Unblock
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  return (
    <Layout>
      <div className="settings-page">
        {/* Header */}
        <header className="settings-page-header reveal reveal-1">
          <span className="kicker">// SETTINGS</span>
          <h1 className="settings-page-title">Account Settings</h1>
          <p className="settings-page-subtitle">Manage your profile, security, and preferences.</p>
        </header>

        <div className="settings-layout reveal reveal-2">
          {/* Sidebar nav */}
          <nav className="settings-sidebar" aria-label="Settings sections">
            {TABS.map(tab => (
              <button
                key={tab.id}
                className={`settings-nav-item${activeTab === tab.id ? ' active' : ''}`}
                onClick={() => setActiveTab(tab.id)}
                aria-current={activeTab === tab.id ? 'page' : undefined}
              >
                <span className="settings-nav-icon" aria-hidden="true">{tab.icon}</span>
                <span>{tab.label}</span>
              </button>
            ))}
          </nav>

          {/* Content area */}
          <div className="settings-content-area">
            {loading ? (
              <div className="settings-loading">
                <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)' }} aria-label="Loading settings" />
              </div>
            ) : (
              <>
                {activeTab === 'profile'       && renderProfile()}
                {activeTab === 'account'       && renderAccount()}
                {activeTab === 'appearance'    && renderAppearance()}
                {activeTab === 'notifications' && renderNotifications()}
                {activeTab === 'privacy'       && renderPrivacy()}
              </>
            )}
          </div>
        </div>
      </div>
    </Layout>
  );
}
