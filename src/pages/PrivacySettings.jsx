import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import { Unavailable, LoadError } from '../components/state/Unavailable';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';

/* Only when VITE_USE_MOCKS === 'true'. This list used to be the unconditional
   initial state, so every user was shown two invented people they had
   supposedly blocked. */
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

const MOCK_BLOCKED_USERS = USE_MOCKS ? [
  { id: 'user_001', username: 'cheater123', blockedDate: '2026-04-10' },
  { id: 'user_002', username: 'toxic_player', blockedDate: '2026-05-02' },
] : [];

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

  /* GET /api/v1/friends/blocked is mounted — read the real list. */
  const [blockedLoading, setBlockedLoading] = useState(!USE_MOCKS);
  const [blockedError, setBlockedError] = useState(null);

  const loadBlocked = useCallback(async () => {
    setBlockedLoading(true);
    setBlockedError(null);
    try {
      const data = await api.social.blocked();
      const list = Array.isArray(data) ? data : (data?.blocked ?? data?.users ?? []);
      setBlockedUsers(
        (Array.isArray(list) ? list : []).map(u => ({
          id: u.id ?? u.user_id,
          username: u.username ?? 'user',
          blockedDate: (u.blocked_at ?? u.created_at ?? '').slice(0, 10) || '—',
        }))
      );
    } catch (err) {
      setBlockedError(err.message || 'Failed to load blocked users');
    } finally {
      setBlockedLoading(false);
    }
  }, []);

  useEffect(() => {
    if (USE_MOCKS) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadBlocked();
  }, [loadBlocked]);

  const handleUnblockUser = async (userId) => {
    try {
      await api.social.unblockUser(userId);
      setBlockedUsers(prev => prev.filter(u => u.id !== userId));
    } catch (err) {
      setBlockedError(err.message || 'Failed to unblock');
    }
  };

  /* Data export and account deletion are NOT implemented on this backend.
   * The old handlers were theatre: export slept for two seconds and resolved,
   * delete just closed its own confirmation dialog. Both are now stated as
   * absent rather than mimed, because a privacy page that pretends to delete an
   * account is the worst possible place to be dishonest. */

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
            {blockedLoading ? (
              <div aria-busy="true"><span className="sk sk-row" /><span className="sk sk-row" /></div>
            ) : blockedError ? (
              <LoadError
                headingLevel={3}
                title="Could not load blocked users"
                detail={blockedError}
                onRetry={loadBlocked}
              >
                Blocking is implemented on this node; this request did not land.
              </LoadError>
            ) : blockedUsers.length === 0 ? (
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
          <Unavailable
            headingLevel={3}
            title="Data export is not built yet"
            endpoints={['POST /api/v1/auth/data-export']}
          >
            This node cannot assemble and hand you an archive of your data — no
            route exists to request one. Until it does, ask the operator of this
            node directly.
          </Unavailable>
        </section>

        {/* Danger zone */}
        <section className="settings-section settings-danger-zone reveal reveal-7" aria-labelledby="priv-danger-heading">
          <h2 id="priv-danger-heading" className="settings-section-title">{t('account.dangerZone')}</h2>
          <Unavailable
            headingLevel={3}
            title="Account deletion is not built yet"
            endpoints={['DELETE /api/v1/auth/me']}
          >
            There is no delete-account route on this node, so nothing here can
            erase your account — and a button that appeared to would be a lie
            about your data. Ask the operator of this node to remove it.
          </Unavailable>
        </section>
      </div>
    </Layout>
  );
}
