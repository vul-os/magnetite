import { useState, useEffect } from 'react';
import type { ChangeEvent, FormEvent } from 'react';
import Layout from '../components/Layout';
import ThemeToggle from '../components/ThemeToggle';
import NotificationPreferences from '../components/NotificationPreferences';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';

interface ProfileData {
  name: string;
  email: string;
  avatar: string;
  bio: string;
  location: string;
}

interface AccountData {
  email: string;
  twoFactorEnabled: boolean;
}

interface PrivacyData {
  profileVisibility: string;
  showOnLeaderboards: boolean;
  blockedUsers: string[];
}

interface SessionItem {
  id: string;
  device: string;
  location: string;
  lastActive: string;
  current: boolean;
}

interface RawSession {
  id?: string;
  session_id?: string;
  user_agent?: string;
  device?: string;
  ip_address?: string;
  location?: string;
  last_active?: string;
  updated_at?: string;
  current?: boolean;
}

/* Mock fallbacks — only when VITE_USE_MOCKS=true */
const MOCK_USER: ProfileData | null = import.meta.env.VITE_USE_MOCKS === 'true'
  ? {
      name: 'StarForge Studios',
      email: 'dev@starforge.com',
      avatar: 'https://picsum.photos/seed/avatar/200/200',
      bio: 'Indie game developer specialising in action and RPG games built in Rust.',
      location: 'San Francisco, CA',
    }
  : null;

const MOCK_SESSIONS: SessionItem[] | null = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 'sess_001', device: 'Chrome on Mac OS',    location: 'San Francisco, CA', lastActive: 'Now',       current: true  },
      { id: 'sess_002', device: 'Safari on iPhone',    location: 'San Francisco, CA', lastActive: '2 hours ago', current: false },
      { id: 'sess_003', device: 'Firefox on Windows',  location: 'New York, NY',       lastActive: 'Yesterday',  current: false },
    ]
  : null;

/* ── Reusable Toggle component ─────────────────────────────────────────────── */
interface ToggleSettingProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

function ToggleSetting({ label, description, checked, onChange }: ToggleSettingProps) {
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
function SaveButton({ saving, success }: { saving: boolean; success: boolean }) {
  const { t } = useTranslation();
  return (
    <button type="submit" className="settings-save-btn" disabled={saving} aria-busy={saving}>
      {saving
        ? <><span className="spinner spinner-sm" aria-hidden="true" />{t('account.saving')}</>
        : success
          ? <><span style={{ color: 'var(--color-success)' }} aria-hidden="true">✓</span>{t('account.saved')}</>
          : t('account.saveChanges')}
    </button>
  );
}

/* ── Tab definitions ─────────────────────────────────────────────────────────── */
const TAB_IDS = ['profile', 'account', 'appearance', 'notifications', 'privacy'];
const TAB_ICONS: Record<string, string> = { profile: '◉', account: '⬡', appearance: '◈', notifications: '◎', privacy: '⊕' };

const DEFAULT_PROFILE: ProfileData = MOCK_USER ?? {
  name: '', email: '', avatar: '', bio: '', location: '',
};

function normaliseSession(s: RawSession): SessionItem {
  return {
    id:         s.id ?? s.session_id ?? '',
    device:     s.user_agent ?? s.device ?? 'Unknown device',
    location:   s.ip_address  ?? s.location ?? 'Unknown',
    lastActive: s.last_active ?? s.updated_at
      ? new Date((s.last_active ?? s.updated_at)!).toLocaleString()
      : 'Unknown',
    current:    s.current ?? false,
  };
}

export default function Settings() {
  const { t } = useTranslation();
  const TABS = TAB_IDS.map(id => ({ id, label: t(`account.settingsTab_${id}`), icon: TAB_ICONS[id] }));
  const [activeTab, setActiveTab] = useState('profile');
  const [profile, setProfile]     = useState<ProfileData>(DEFAULT_PROFILE);
  const [account, setAccount]     = useState<AccountData>({ email: DEFAULT_PROFILE.email, twoFactorEnabled: false });
  // notifications state is managed by <NotificationPreferences /> — no local state needed.
  const [privacy, setPrivacy] = useState<PrivacyData>({
    profileVisibility: 'public',
    showOnLeaderboards: true,
    blockedUsers: [],
  });
  const [saving, setSaving]           = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError]     = useState<string | null>(null);
  const [sessions, setSessions]       = useState<SessionItem[]>(MOCK_SESSIONS ?? []);
  const [loading, setLoading]         = useState(true);
  const [sessionsLoading, setSessionsLoading] = useState(false);
  const [sessionNotice, setSessionNotice] = useState<string | null>(null);
  const [revokingSessionId, setRevokingSessionId] = useState<string | null>(null);

  useEffect(() => {
    async function loadData() {
      setLoading(true);
      try {
        /* Load profile */
        const userData = await api.auth.me() as { user?: Record<string, unknown> } | null;
        if (userData?.user) {
          const u = userData.user;
          setProfile(prev => ({
            ...prev,
            name:     (u.username as string | undefined) ?? (u.name as string | undefined) ?? prev.name,
            email:    (u.email as string | undefined)    ?? prev.email,
            avatar:   (u.avatar as string | undefined)   ?? prev.avatar,
            bio:      (u.bio as string | undefined)      ?? prev.bio,
            location: (u.location as string | undefined) ?? prev.location,
          }));
          setAccount(prev => ({ ...prev, email: (u.email as string | undefined) ?? prev.email }));
        }
      } catch {
        /* use defaults — not an error worth surfacing */
      } finally {
        setLoading(false);
      }
    }
    void loadData();
  }, []);

  useEffect(() => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    async function loadSessions() {
      setSessionsLoading(true);
      try {
        const data = await api.auth.sessions() as { sessions?: RawSession[] } | RawSession[] | null;
        const raw = (data && !Array.isArray(data) ? data.sessions : data) ?? [];
        setSessions(Array.isArray(raw) ? raw.map(normaliseSession) : []);
      } catch {
        /* leave mock sessions */
      } finally {
        setSessionsLoading(false);
      }
    }
    void loadSessions();
  }, []);

  const handleSave = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setSaving(true);
    setSaveError(null);
    try {
      await api.profile.update({
        username: profile.name,
        bio:      profile.bio,
        location: profile.location,
        avatar:   profile.avatar,
      });
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2500);
    } catch (err) {
      setSaveError((err instanceof Error && err.message) || 'Failed to save profile');
    } finally {
      setSaving(false);
    }
  };

  const handleAvatarUpload = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onloadend = () => setProfile(prev => ({ ...prev, avatar: reader.result as string }));
    reader.readAsDataURL(file);
  };

  const handleRevokeSession = async (sessionId: string) => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') {
      // Mock mode runs with no backend (see useAuth.ts) — there is no route to
      // call, so just drop the row from local state like every other mock
      // mutation in this file.
      setSessions(prev => prev.filter(s => s.id !== sessionId));
      return;
    }
    setSessionNotice(null);
    setRevokingSessionId(sessionId);
    try {
      await api.auth.revokeSession(sessionId);
      setSessions(prev => prev.filter(s => s.id !== sessionId));
    } catch (err) {
      setSessionNotice((err instanceof Error && err.message) || 'Failed to revoke session');
    } finally {
      setRevokingSessionId(null);
    }
  };

  /* ── Tab panels ─────────────────────────────────────────────────────────── */

  const renderProfile = () => (
    <form className="settings-tab-panel" onSubmit={handleSave} aria-label={t('account.profileFormLabel')}>
      {saveError && (
        <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
          <span className="auth-error-icon" aria-hidden="true">!</span>
          {saveError}
        </div>
      )}

      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.profileInformation')}</h3>
        <p className="settings-section-desc">{t('account.profileInfoDesc')}</p>

        {/* Avatar */}
        <div className="settings-avatar-row">
          <div className="settings-avatar-wrap">
            {profile.avatar ? (
              <img src={profile.avatar} alt={t('account.yourAvatar')} className="settings-avatar" loading="lazy" />
            ) : (
              <div className="settings-avatar" style={{ background: 'var(--color-bg-elevated)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '2rem' }} aria-label={t('account.avatarInitial')}>
                {profile.name ? profile.name.charAt(0).toUpperCase() : '?'}
              </div>
            )}
            <div className="settings-avatar-overlay" aria-hidden="true">
              <span>{t('account.change')}</span>
            </div>
            <label className="settings-avatar-input-label" aria-label={t('account.uploadNewAvatar')}>
              <input type="file" accept="image/*" onChange={handleAvatarUpload} style={{ display: 'none' }} />
            </label>
          </div>
          <div className="settings-avatar-meta">
            <span className="settings-field-label">{t('account.avatarLabel')}</span>
            <p className="settings-avatar-hint">{t('account.avatarHint')}</p>
          </div>
        </div>

        <div className="settings-grid-2">
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="set-username">{t('auth.usernameLabel')}</label>
            <input
              id="set-username"
              type="text"
              value={profile.name}
              onChange={(e) => setProfile({ ...profile, name: e.target.value })}
              className="settings-input"
            />
          </div>
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="set-location">{t('account.locationLabel')}</label>
            <input
              id="set-location"
              type="text"
              value={profile.location}
              onChange={(e) => setProfile({ ...profile, location: e.target.value })}
              className="settings-input"
              placeholder={t('account.locationPlaceholder')}
            />
          </div>
        </div>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="set-bio">{t('account.bioLabel')}</label>
          <textarea
            id="set-bio"
            value={profile.bio}
            onChange={(e) => setProfile({ ...profile, bio: e.target.value })}
            className="settings-input settings-textarea"
            rows={3}
            placeholder={t('account.bioPlaceholder')}
          />
        </div>
      </div>
      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  const renderAccount = () => (
    <form className="settings-tab-panel" onSubmit={handleSave} aria-label={t('account.accountFormLabel')}>
      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.emailAddress')}</h3>
        <div className="settings-input-action">
          <input
            type="email"
            value={account.email}
            onChange={(e) => setAccount({ ...account, email: e.target.value })}
            className="settings-input"
            aria-label={t('auth.emailLabel')}
          />
          <button type="button" className="settings-action-btn">{t('account.changeEmail')}</button>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.passwordSection')}</h3>
        <div className="settings-grid-2">
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="cur-pw-set">{t('account.currentPassword')}</label>
            <input id="cur-pw-set" type="password" className="settings-input" placeholder={t('account.enterCurrentPassword')} autoComplete="current-password" />
          </div>
          <div />
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="new-pw-set">{t('auth.newPasswordLabel')}</label>
            <input id="new-pw-set" type="password" className="settings-input" placeholder={t('auth.newPasswordPlaceholder')} autoComplete="new-password" />
          </div>
          <div className="settings-field">
            <label className="settings-field-label" htmlFor="conf-pw-set">{t('account.confirmNewPassword')}</label>
            <input id="conf-pw-set" type="password" className="settings-input" placeholder={t('auth.confirmPasswordPlaceholder')} autoComplete="new-password" />
          </div>
        </div>
        <button type="button" className="settings-action-btn">{t('account.updatePassword')}</button>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.twoFactorAuth')}</h3>
        <ToggleSetting
          label={t('account.enable2FA')}
          description={t('account.enable2FADesc')}
          checked={account.twoFactorEnabled}
          onChange={(v) => setAccount({ ...account, twoFactorEnabled: v })}
        />
      </div>

      <div className="settings-section">
        <div className="settings-section-head">
          <div>
            <h3 className="settings-section-title">{t('account.activeSessions')}</h3>
            <p className="settings-section-desc">{t('account.activeSessionsDesc')}</p>
          </div>
        </div>
        {sessionNotice && (
          <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {sessionNotice}
          </div>
        )}
        <div className="settings-sessions">
          {sessionsLoading ? (
            <div style={{ padding: '1rem', color: 'var(--color-text-muted)' }} aria-busy="true">
              <span className="spinner spinner-sm" aria-hidden="true" /> {t('account.loadingSessions')}
            </div>
          ) : sessions.length === 0 ? (
            <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>
              {t('account.noActiveSessions')}
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
                      <span className="settings-session-badge">{t('account.currentSession')}</span>
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
                    disabled={revokingSessionId === session.id}
                    aria-label={t('account.revokeSessionLabel', { device: session.device })}
                  >
                    {revokingSessionId === session.id ? t('account.revoking') : t('account.revoke')}
                  </button>
                )}
              </div>
            ))
          )}
        </div>
      </div>

      <SaveButton saving={saving} success={saveSuccess} />
    </form>
  );

  const renderAppearance = () => (
    <div className="settings-tab-panel">
      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.theme')}</h3>
        <p className="settings-section-desc">{t('account.themeDesc')}</p>
        <div className="settings-theme-row">
          <ThemeToggle />
        </div>
      </div>
    </div>
  );

  const renderNotifications = () => (
    <div className="settings-tab-panel" aria-label={t('account.notificationsFormLabel')}>
      {/*
        NotificationPreferences handles its own GET/PUT lifecycle against
        GET /api/v1/notifications/preferences and PUT /api/v1/notifications/preferences.
        It is self-contained — no save propagation needed from this parent.
      */}
      <NotificationPreferences />
    </div>
  );

  const renderPrivacy = () => (
    <form className="settings-tab-panel" onSubmit={handleSave} aria-label={t('account.privacyFormLabel')}>
      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.profileVisibility')}</h3>
        <div className="settings-field" style={{ maxWidth: 320 }}>
          <label className="settings-field-label" htmlFor="prof-vis">{t('account.whoCanView')}</label>
          <select
            id="prof-vis"
            value={privacy.profileVisibility}
            onChange={(e) => setPrivacy({ ...privacy, profileVisibility: e.target.value })}
            className="settings-input"
          >
            <option value="public">{t('account.visibilityPublic')}</option>
            <option value="friends">{t('account.visibilityFriends')}</option>
            <option value="private">{t('account.visibilityPrivate')}</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">{t('account.visibilityOptions')}</h3>
        <ToggleSetting
          label={t('account.showOnLeaderboards')}
          description={t('account.showOnLeaderboardsDesc')}
          checked={privacy.showOnLeaderboards}
          onChange={(v) => setPrivacy({ ...privacy, showOnLeaderboards: v })}
        />
      </div>

      {privacy.blockedUsers.length > 0 && (
        <div className="settings-section">
          <h3 className="settings-section-title">{t('account.blockedUsers')}</h3>
          <p className="settings-section-desc">{t('account.blockedUsersDesc')}</p>
          <div className="settings-blocked-list">
            {privacy.blockedUsers.map(userId => (
              <div key={userId} className="settings-blocked-item">
                <span>{t('account.blockedUserLabel', { id: userId })}</span>
                <button
                  type="button"
                  className="settings-action-btn"
                  onClick={() => setPrivacy(prev => ({ ...prev, blockedUsers: prev.blockedUsers.filter(id => id !== userId) }))}
                >
                  {t('account.unblock')}
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
          <h1 className="settings-page-title">{t('account.settingsTitle')}</h1>
          <p className="settings-page-subtitle">{t('account.settingsSubtitle')}</p>
        </header>

        <div className="settings-layout reveal reveal-2">
          {/* Sidebar nav */}
          <nav className="settings-sidebar" aria-label={t('account.settingsNavLabel')}>
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
          <div className="settings-content-area" role="region" aria-label={t('account.settingsContent')}>
            {loading ? (
              <div className="settings-loading">
                <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)' }} aria-label={t('account.loadingSettings')} />
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
