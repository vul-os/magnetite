import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import ThemeToggle from '../components/ThemeToggle';
import { api } from '../api/client';

const MOCK_USER = {
  name: 'StarForge Studios',
  email: 'dev@starforge.com',
  avatar: 'https://picsum.photos/seed/avatar/200/200',
  bio: 'Indie game developer specializing in action and RPG games.',
  location: 'San Francisco, CA',
};

const MOCK_SESSIONS = [
  { id: 'sess_001', device: 'Chrome on Mac OS', location: 'San Francisco, CA', lastActive: 'Now', current: true },
  { id: 'sess_002', device: 'Safari on iPhone', location: 'San Francisco, CA', lastActive: '2 hours ago', current: false },
  { id: 'sess_003', device: 'Firefox on Windows', location: 'New York, NY', lastActive: 'Yesterday', current: false },
];

export default function Settings() {
  const [activeTab, setActiveTab] = useState('profile');
  const [profile, setProfile] = useState(MOCK_USER);
  const [account, setAccount] = useState({ email: MOCK_USER.email, twoFactorEnabled: false });
  const [notifications, setNotifications] = useState({
    email: { promotions: true, updates: true, newsletter: false },
    push: { matches: true, friends: true, system: false },
    frequency: 'instant',
  });
  const [privacy, setPrivacy] = useState({
    profileVisibility: 'public',
    showOnLeaderboards: true,
    blockedUsers: [],
  });
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [sessions, setSessions] = useState(MOCK_SESSIONS);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadData() {
      try {
        const userData = await api.auth.me();
        if (userData.user) {
          setProfile(prev => ({ ...prev, ...userData.user }));
        }
      } catch (err) {
        console.log('Using mock profile data');
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
      await new Promise(resolve => setTimeout(resolve, 1000));
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } catch (err) {
      console.error('Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  const handleAvatarUpload = (e) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onloadend = () => {
        setProfile(prev => ({ ...prev, avatar: reader.result }));
      };
      reader.readAsDataURL(file);
    }
  };

  const handleRevokeSession = (sessionId) => {
    setSessions(prev => prev.filter(s => s.id !== sessionId));
  };

  const handleUnblockUser = (userId) => {
    setPrivacy(prev => ({
      ...prev,
      blockedUsers: prev.blockedUsers.filter(id => id !== userId),
    }));
  };

  const tabs = [
    { id: 'profile', label: 'Profile', icon: '👤' },
    { id: 'account', label: 'Account', icon: '🔐' },
    { id: 'appearance', label: 'Appearance', icon: '🎨' },
    { id: 'notifications', label: 'Notifications', icon: '🔔' },
    { id: 'privacy', label: 'Privacy', icon: '🛡️' },
  ];

  const renderProfile = () => (
    <form className="settings-form" onSubmit={handleSave}>
      <div className="form-section">
        <h3>Profile Information</h3>
        <div className="avatar-section">
          <img src={profile.avatar} alt="Avatar" className="avatar-preview" loading="lazy" />
          <div className="avatar-actions">
            <label className="btn btn-secondary" style={{ cursor: 'pointer' }}>
              Upload Avatar
              <input type="file" accept="image/*" onChange={handleAvatarUpload} style={{ display: 'none' }} />
            </label>
            <p className="avatar-hint">Recommended: 200x200px, JPG or PNG</p>
          </div>
        </div>

        <div className="form-row">
          <div className="form-group">
            <label>Username</label>
            <input
              type="text"
              value={profile.name}
              onChange={(e) => setProfile({ ...profile, name: e.target.value })}
            />
          </div>
          <div className="form-group">
            <label>Location</label>
            <input
              type="text"
              value={profile.location}
              onChange={(e) => setProfile({ ...profile, location: e.target.value })}
              placeholder="City, Country"
            />
          </div>
        </div>

        <div className="form-group">
          <label>Bio</label>
          <textarea
            value={profile.bio}
            onChange={(e) => setProfile({ ...profile, bio: e.target.value })}
            rows={4}
            placeholder="Tell us about yourself..."
          />
        </div>
      </div>

      <button type="submit" className="btn btn-primary" disabled={saving}>
        {saving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save Changes'}
      </button>
    </form>
  );

  const renderAppearance = () => (
    <div className="settings-form">
      <div className="form-section">
        <h3>Theme</h3>
        <p className="section-description">Choose your preferred color scheme</p>
        <div className="theme-selector">
          <ThemeToggle />
        </div>
      </div>
    </div>
  );

  const renderAccount = () => (
    <form className="settings-form" onSubmit={handleSave}>
      <div className="form-section">
        <h3>Email</h3>
        <div className="form-group">
          <label>Email Address</label>
          <div className="input-with-action">
            <input
              type="email"
              value={account.email}
              onChange={(e) => setAccount({ ...account, email: e.target.value })}
            />
            <button type="button" className="btn btn-secondary">Change Email</button>
          </div>
        </div>
      </div>

      <div className="form-section">
        <h3>Password</h3>
        <div className="form-group">
          <label>Current Password</label>
          <input type="password" placeholder="Enter current password" />
        </div>
        <div className="form-row">
          <div className="form-group">
            <label>New Password</label>
            <input type="password" placeholder="Enter new password" />
          </div>
          <div className="form-group">
            <label>Confirm New Password</label>
            <input type="password" placeholder="Confirm new password" />
          </div>
        </div>
        <button type="button" className="btn btn-secondary">Update Password</button>
      </div>

      <div className="form-section">
        <h3>Two-Factor Authentication</h3>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Enable 2FA</span>
            <p className="toggle-description">Add an extra layer of security to your account</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={account.twoFactorEnabled}
              onChange={(e) => setAccount({ ...account, twoFactorEnabled: e.target.checked })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="form-section">
        <h3>Active Sessions</h3>
        <p className="section-description">Manage your active sessions across devices</p>
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
                  type="button"
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

      <button type="submit" className="btn btn-primary" disabled={saving}>
        {saving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save Changes'}
      </button>
    </form>
  );

  const renderNotifications = () => (
    <form className="settings-form" onSubmit={handleSave}>
      <div className="form-section">
        <h3>Email Notifications</h3>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Promotional Emails</span>
            <p className="toggle-description">Receive updates about new games and special offers</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.email.promotions}
              onChange={(e) => setNotifications({
                ...notifications,
                email: { ...notifications.email, promotions: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Account Updates</span>
            <p className="toggle-description">Important updates about your account and security</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.email.updates}
              onChange={(e) => setNotifications({
                ...notifications,
                email: { ...notifications.email, updates: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Newsletter</span>
            <p className="toggle-description">Weekly digest and community updates</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.email.newsletter}
              onChange={(e) => setNotifications({
                ...notifications,
                email: { ...notifications.email, newsletter: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="form-section">
        <h3>Push Notifications</h3>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Match Notifications</span>
            <p className="toggle-description">When you find a match for multiplayer games</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.push.matches}
              onChange={(e) => setNotifications({
                ...notifications,
                push: { ...notifications.push, matches: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Friend Activity</span>
            <p className="toggle-description">When friends join, leave, or go online</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.push.friends}
              onChange={(e) => setNotifications({
                ...notifications,
                push: { ...notifications.push, friends: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">System Notifications</span>
            <p className="toggle-description">General system announcements</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notifications.push.system}
              onChange={(e) => setNotifications({
                ...notifications,
                push: { ...notifications.push, system: e.target.checked }
              })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="form-section">
        <h3>Notification Frequency</h3>
        <div className="form-group">
          <label>Email Frequency</label>
          <select
            value={notifications.frequency}
            onChange={(e) => setNotifications({ ...notifications, frequency: e.target.value })}
          >
            <option value="instant">Instant</option>
            <option value="hourly">Hourly Digest</option>
            <option value="daily">Daily Digest</option>
            <option value="weekly">Weekly Digest</option>
            <option value="never">Never</option>
          </select>
        </div>
      </div>

      <button type="submit" className="btn btn-primary" disabled={saving}>
        {saving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save Changes'}
      </button>
    </form>
  );

  const renderPrivacy = () => (
    <form className="settings-form" onSubmit={handleSave}>
      <div className="form-section">
        <h3>Profile Visibility</h3>
        <div className="form-group">
          <label>Who can see your profile</label>
          <select
            value={privacy.profileVisibility}
            onChange={(e) => setPrivacy({ ...privacy, profileVisibility: e.target.value })}
          >
            <option value="public">Public - Anyone can view your profile</option>
            <option value="friends">Friends Only - Only friends can view your profile</option>
            <option value="private">Private - Only you can view your profile</option>
          </select>
        </div>
      </div>

      <div className="form-section">
        <h3>Leaderboard</h3>
        <div className="toggle-setting">
          <div>
            <span className="toggle-label">Show on Leaderboards</span>
            <p className="toggle-description">Appear on public leaderboards with your scores</p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={privacy.showOnLeaderboards}
              onChange={(e) => setPrivacy({ ...privacy, showOnLeaderboards: e.target.checked })}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="form-section">
        <h3>Blocked Users</h3>
        <p className="section-description">Users you've blocked cannot see your profile or send you messages</p>
        {privacy.blockedUsers.length === 0 ? (
          <p className="empty-state">You haven't blocked any users</p>
        ) : (
          <div className="blocked-users-list">
            {privacy.blockedUsers.map(userId => (
              <div key={userId} className="blocked-user-item">
                <span>Blocked User</span>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => handleUnblockUser(userId)}
                >
                  Unblock
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <button type="submit" className="btn btn-primary" disabled={saving}>
        {saving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save Changes'}
      </button>
    </form>
  );

  return (
    <Layout>
      <div className="settings-page">
        <header className="settings-header">
          <h1>Settings</h1>
          <p>Manage your account settings and preferences</p>
        </header>

        <div className="settings-layout">
          <nav className="settings-nav">
            {tabs.map(tab => (
              <button
                key={tab.id}
                className={`nav-item ${activeTab === tab.id ? 'active' : ''}`}
                onClick={() => setActiveTab(tab.id)}
              >
                <span className="nav-icon">{tab.icon}</span>
                <span>{tab.label}</span>
              </button>
            ))}
          </nav>

          <div className="settings-content">
            {loading ? (
              <div className="loading-state">Loading settings...</div>
            ) : (
              <>
                {activeTab === 'profile' && renderProfile()}
                {activeTab === 'account' && renderAccount()}
                {activeTab === 'appearance' && renderAppearance()}
                {activeTab === 'notifications' && renderNotifications()}
                {activeTab === 'privacy' && renderPrivacy()}
              </>
            )}
          </div>
        </div>
      </div>
    </Layout>
  );
}