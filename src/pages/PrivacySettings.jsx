import { useState } from 'react';
import Layout from '../components/Layout';
import { useTranslation } from '../i18n/useTranslation';

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
  const { t } = useTranslation();
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
          <h1 className="settings-page-title">{t('account.privacyTitle')}</h1>
          <p className="settings-page-subtitle">{t('account.privacySubtitle')}</p>
        </header>

        <form onSubmit={handleSave} aria-label={t('account.privacyFormLabel')}>
          {/* Profile Visibility */}
          <section className="settings-section reveal reveal-2" aria-labelledby="priv-vis-heading">
            <h2 id="priv-vis-heading" className="settings-section-title">{t('account.profileVisibility')}</h2>
            <div className="settings-field" style={{ maxWidth: 340, marginBottom: 0 }}>
              <label className="settings-field-label" htmlFor="priv-vis">{t('account.whoCanView')}</label>
              <select
                id="priv-vis"
                value={privacy.profileVisibility}
                onChange={(e) => setPrivacy({ ...privacy, profileVisibility: e.target.value })}
                className="settings-input"
              >
                <option value="public">{t('account.visibilityPublic')}</option>
                <option value="friends">{t('account.visibilityFriends')}</option>
                <option value="private">{t('account.visibilityPrivate')}</option>
              </select>
            </div>
          </section>

          {/* Visibility options */}
          <section className="settings-section reveal reveal-3" aria-labelledby="priv-options-heading">
            <h2 id="priv-options-heading" className="settings-section-title">{t('account.visibilityOptions')}</h2>
            <ToggleSetting
              label={t('account.showOnLeaderboards')}
              description={t('account.showOnLeaderboardsDesc')}
              checked={privacy.showOnLeaderboards}
              onChange={(v) => setPrivacy({ ...privacy, showOnLeaderboards: v })}
            />
            <ToggleSetting
              label={t('account.showOnlineStatus')}
              description={t('account.showOnlineStatusDesc')}
              checked={privacy.showOnlineStatus}
              onChange={(v) => setPrivacy({ ...privacy, showOnlineStatus: v })}
            />
            <ToggleSetting
              label={t('account.allowFriendRequests')}
              description={t('account.allowFriendRequestsDesc')}
              checked={privacy.allowFriendRequests}
              onChange={(v) => setPrivacy({ ...privacy, allowFriendRequests: v })}
            />
          </section>

          {/* Blocked users */}
          <section className="settings-section reveal reveal-4" aria-labelledby="priv-blocked-heading">
            <h2 id="priv-blocked-heading" className="settings-section-title">{t('account.blockedUsers')}</h2>
            <p className="settings-section-desc">{t('account.blockedUsersDesc')}</p>
            {blockedUsers.length === 0 ? (
              <p style={{ fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-muted)', margin: 0 }}>
                {t('account.noBlockedUsers')}
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
                        {t('account.blockedOn', { date: user.blockedDate })}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="settings-action-btn"
                      onClick={() => handleUnblockUser(user.id)}
                      aria-label={t('account.unblockUserLabel', { username: user.username })}
                    >
                      {t('account.unblock')}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>

          <button
            type="submit"
            className="settings-save-btn reveal reveal-5"
            disabled={saving}
            aria-busy={saving}
            style={{ marginBottom: '2rem' }}
          >
            {saving
              ? <><span className="spinner spinner-sm" aria-hidden="true" />{t('account.saving')}</>
              : saveSuccess
                ? <><span style={{ color: 'var(--color-success)' }} aria-hidden="true">✓</span>{t('account.saved')}</>
                : t('account.saveChanges')}
          </button>
        </form>

        {/* Data Management */}
        <section className="settings-section reveal reveal-6" aria-labelledby="priv-data-heading">
          <h2 id="priv-data-heading" className="settings-section-title">{t('account.dataManagement')}</h2>
          <div className="settings-danger-row">
            <div>
              <span className="settings-danger-action-title">{t('account.exportYourData')}</span>
              <p className="settings-danger-desc">{t('account.exportYourDataDesc')}</p>
            </div>
            <button
              type="button"
              className="settings-action-btn"
              onClick={handleExportData}
              disabled={exporting}
              aria-busy={exporting}
            >
              {exporting
                ? <><span className="spinner spinner-sm" aria-hidden="true" /><span className="sr-only">{t('account.exportingData')}</span></>
                : t('account.requestExport')}
            </button>
          </div>
        </section>

        {/* Danger zone */}
        <section className="settings-section settings-danger-zone reveal reveal-7" aria-labelledby="priv-danger-heading">
          <h2 id="priv-danger-heading" className="settings-section-title">{t('account.dangerZone')}</h2>
          <div className="settings-danger-row">
            <div>
              <span className="settings-danger-action-title">{t('account.deleteAccount')}</span>
              <p className="settings-danger-desc">{t('account.deleteAccountDesc')}</p>
            </div>
            {showDeleteConfirm ? (
              <div style={{ display: 'flex', gap: '0.625rem', flexWrap: 'wrap' }}>
                <button
                  type="button"
                  className="settings-revoke-btn"
                  style={{ padding: '0.5rem 1rem', border: '1px solid var(--color-error)', background: 'rgba(255,84,104,0.1)', color: 'var(--color-error)' }}
                  onClick={handleDeleteAccount}
                >
                  {t('account.confirmDelete')}
                </button>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => setShowDeleteConfirm(false)}
                >
                  {t('common.cancel')}
                </button>
              </div>
            ) : (
              <button
                type="button"
                className="settings-revoke-btn"
                style={{ padding: '0.5rem 1rem', border: '1px solid rgba(255,84,104,0.4)', color: 'var(--color-error)' }}
                onClick={() => setShowDeleteConfirm(true)}
              >
                {t('account.deleteAccount')}
              </button>
            )}
          </div>
        </section>
      </div>
    </Layout>
  );
}
