import { useState } from 'react';
import Layout from '../components/Layout';

const MOCK_BLOCKED_USERS = [
  { id: 'user_001', username: 'cheater123', blockedDate: '2026-04-10' },
  { id: 'user_002', username: 'toxic_player', blockedDate: '2026-05-02' },
];

export default function Privacy() {
  const [privacy, setPrivacy] = useState({
    profileVisibility: 'public',
    showOnLeaderboards: true,
    showOnlineStatus: true,
    allowFriendRequests: true,
  });
  const [blockedUsers, setBlockedUsers] = useState(MOCK_BLOCKED_USERS);
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [exporting, setExporting] = useState(false);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 1000));
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } catch {
      console.error('Failed to save privacy settings');
    } finally {
      setSaving(false);
    }
  };

  const handleUnblockUser = (userId) => {
    setBlockedUsers(prev => prev.filter(u => u.id !== userId));
  };

  const handleExportData = async () => {
    setExporting(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 2000));
      alert('Data export has been initiated. You will receive an email when your data is ready for download.');
    } catch {
      console.error('Failed to export data');
    } finally {
      setExporting(false);
    }
  };

  const handleDeleteAccount = () => {
    alert('Account deletion has been initiated. You will receive an email with confirmation instructions.');
    setShowDeleteConfirm(false);
  };

  return (
    <Layout>
      <div className="settings-page privacy-page">
        <header className="settings-header">
          <h1>Privacy</h1>
          <p>Control your privacy settings and manage your data</p>
        </header>

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
            <h3>Visibility Options</h3>
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
            <div className="toggle-setting">
              <div>
                <span className="toggle-label">Show Online Status</span>
                <p className="toggle-description">Let others see when you're online</p>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={privacy.showOnlineStatus}
                  onChange={(e) => setPrivacy({ ...privacy, showOnlineStatus: e.target.checked })}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="toggle-setting">
              <div>
                <span className="toggle-label">Allow Friend Requests</span>
                <p className="toggle-description">Let others send you friend requests</p>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={privacy.allowFriendRequests}
                  onChange={(e) => setPrivacy({ ...privacy, allowFriendRequests: e.target.checked })}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>

          <div className="form-section">
            <h3>Blocked Users</h3>
            <p className="section-description">Users you've blocked cannot see your profile or send you messages</p>
            {blockedUsers.length === 0 ? (
              <p className="empty-state">You haven't blocked any users</p>
            ) : (
              <div className="blocked-users-list">
                {blockedUsers.map(user => (
                  <div key={user.id} className="blocked-user-item">
                    <div className="blocked-user-info">
                      <span className="blocked-username">@{user.username}</span>
                      <span className="blocked-date">Blocked on {user.blockedDate}</span>
                    </div>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => handleUnblockUser(user.id)}
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

        <div className="form-section danger-zone">
          <h3>Data Management</h3>
          <div className="danger-action">
            <div>
              <span className="action-title">Export Your Data</span>
              <p className="action-description">Download a copy of all your data</p>
            </div>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={handleExportData}
              disabled={exporting}
            >
              {exporting ? 'Exporting...' : 'Request Export'}
            </button>
          </div>
        </div>

        <div className="form-section danger-zone">
          <h3>Danger Zone</h3>
          <div className="danger-action">
            <div>
              <span className="action-title">Delete Account</span>
              <p className="action-description">Permanently delete your account and all data</p>
            </div>
            {showDeleteConfirm ? (
              <div className="confirm-delete">
                <p className="confirm-text">Are you sure? This action cannot be undone.</p>
                <div className="confirm-buttons">
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={handleDeleteAccount}
                  >
                    Yes, Delete Account
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowDeleteConfirm(false)}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => setShowDeleteConfirm(true)}
              >
                Delete Account
              </button>
            )}
          </div>
        </div>
      </div>
    </Layout>
  );
}