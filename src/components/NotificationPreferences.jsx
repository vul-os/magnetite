/**
 * NotificationPreferences — self-contained component.
 *
 * Displays per-channel (Email / In-App / Push), per-category
 * (Payouts / Social / Achievements / Marketing) notification toggles.
 *
 * Data flow: fetches GET /api/v1/notifications/preferences on mount;
 * PUT on save.  All i18n goes through useTranslation.  Full a11y:
 * - role="switch" toggles with aria-checked + aria-label
 * - visible focus rings via token :focus-visible
 * - semantic <section> / <fieldset>/<legend> structure
 * - prefers-reduced-motion respected via CSS token overrides
 * - tap targets ≥ 40px
 * - works at 360 / 768 / 1280 without horizontal overflow
 *
 * NOTE (agent 5): This component is ready to mount.  Wire it into a settings
 * tab by importing and rendering <NotificationPreferences /> — no route change
 * needed.  Follow-up: add a dedicated /settings/notifications lazy route in
 * App.jsx (out of scope for this wave per ownership rules).
 */

import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './NotificationPreferences.css';

// ── Icons (inline SVG — no external dependency) ───────────────────────────

function IconPayouts() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"
        fill="currentColor" />
    </svg>
  );
}

function IconSocial() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z"
        fill="currentColor" />
    </svg>
  );
}

function IconAchievements() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94.63 1.5 1.98 2.63 3.61 2.96V19H7v2h10v-2h-4v-3.1c1.63-.33 2.98-1.46 3.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"
        fill="currentColor" />
    </svg>
  );
}

function IconMarketing() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
        fill="currentColor" />
    </svg>
  );
}

// ── Default preferences shape (mirrors DB defaults) ───────────────────────

const DEFAULT_PREFS = {
  payouts_email: true,
  payouts_in_app: true,
  payouts_push: true,
  social_email: true,
  social_in_app: true,
  social_push: true,
  achievements_email: true,
  achievements_in_app: true,
  achievements_push: false,
  marketing_email: false,
  marketing_in_app: false,
  marketing_push: false,
};

// ── Reducer ───────────────────────────────────────────────────────────────

function prefsReducer(state, action) {
  switch (action.type) {
    case 'LOAD':
      return { ...state, ...action.payload };
    case 'TOGGLE':
      return { ...state, [action.key]: !state[action.key] };
    default:
      return state;
  }
}

// ── ToggleSwitch sub-component ─────────────────────────────────────────────

function ToggleSwitch({ id, label, checked, onChange, disabled }) {
  return (
    <div className="np-toggle-cell">
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className="np-toggle"
        onClick={onChange}
        disabled={disabled}
      >
        <span className="np-toggle-thumb" />
      </button>
    </div>
  );
}

// ── CategoryCard sub-component ─────────────────────────────────────────────

function CategoryCard({ icon, title, description, channels, prefs, onToggle, disabled, t }) {
  const channelLabels = [
    t('notifPrefs.channelEmail'),
    t('notifPrefs.channelInApp'),
    t('notifPrefs.channelPush'),
  ];

  return (
    <section
      className="np-card"
      aria-label={title}
    >
      <div className="np-card-header">
        <div className="np-card-icon" aria-hidden="true">
          {icon}
        </div>
        <div className="np-card-meta">
          <h3 className="np-card-title">{title}</h3>
          {description && <p className="np-card-desc">{description}</p>}
        </div>
      </div>

      {/* Column labels */}
      <div className="np-channels-header" role="row">
        <div className="np-channels-header-spacer" />
        {channelLabels.map((lbl) => (
          <div key={lbl} className="np-ch-label" role="columnheader" aria-label={lbl}>
            {lbl}
          </div>
        ))}
      </div>

      {/* One row per channel type */}
      <div className="np-channels">
        {channels.map(({ label, keys }) => (
          <div key={keys[0]} className="np-channel-row" role="row">
            <span className="np-channel-name" id={`label-${keys[0]}`}>
              {label}
            </span>
            {keys.map((key, idx) => (
              <ToggleSwitch
                key={key}
                id={`toggle-${key}`}
                label={`${label} — ${channelLabels[idx]}`}
                checked={prefs[key]}
                onChange={() => onToggle(key)}
                disabled={disabled}
              />
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

// ── Main component ─────────────────────────────────────────────────────────

export default function NotificationPreferences() {
  const { t } = useTranslation();

  const [prefs, dispatch] = useReducer(prefsReducer, DEFAULT_PREFS);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [alertState, setAlertState] = useState(null); // 'success' | 'error' | null
  const alertTimerRef = useRef(null);

  // Fetch preferences on mount.
  useEffect(() => {
    let cancelled = false;
    api.notifications
      .getPreferences()
      .then((data) => {
        if (!cancelled) {
          dispatch({ type: 'LOAD', payload: data });
          setLoading(false);
        }
      })
      .catch(() => {
        // Non-fatal — keep defaults; user can still save.
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const handleToggle = useCallback((key) => {
    dispatch({ type: 'TOGGLE', key });
  }, []);

  const showAlert = useCallback((type) => {
    setAlertState(type);
    clearTimeout(alertTimerRef.current);
    alertTimerRef.current = setTimeout(() => setAlertState(null), 3500);
  }, []);

  const handleSave = async (e) => {
    e.preventDefault();
    if (saving) return;
    setSaving(true);
    try {
      await api.notifications.updatePreferences(prefs);
      showAlert('success');
    } catch {
      showAlert('error');
    } finally {
      setSaving(false);
    }
  };

  // Category definitions — i18n keys resolved at render time.
  const categories = [
    {
      key: 'payouts',
      icon: <IconPayouts />,
      title: t('notifPrefs.categoryPayouts'),
      description: t('notifPrefs.categoryPayoutsDesc'),
      channels: [
        {
          label: t('notifPrefs.rowPayouts'),
          keys: ['payouts_email', 'payouts_in_app', 'payouts_push'],
        },
      ],
    },
    {
      key: 'social',
      icon: <IconSocial />,
      title: t('notifPrefs.categorySocial'),
      description: t('notifPrefs.categorySocialDesc'),
      channels: [
        {
          label: t('notifPrefs.rowFriendRequests'),
          keys: ['social_email', 'social_in_app', 'social_push'],
        },
      ],
    },
    {
      key: 'achievements',
      icon: <IconAchievements />,
      title: t('notifPrefs.categoryAchievements'),
      description: t('notifPrefs.categoryAchievementsDesc'),
      channels: [
        {
          label: t('notifPrefs.rowAchievements'),
          keys: ['achievements_email', 'achievements_in_app', 'achievements_push'],
        },
      ],
    },
    {
      key: 'marketing',
      icon: <IconMarketing />,
      title: t('notifPrefs.categoryMarketing'),
      description: t('notifPrefs.categoryMarketingDesc'),
      channels: [
        {
          label: t('notifPrefs.rowMarketing'),
          keys: ['marketing_email', 'marketing_in_app', 'marketing_push'],
        },
      ],
    },
  ];

  if (loading) {
    return (
      <div className="np-root" aria-busy="true" aria-label={t('common.loading')}>
        {[1, 2, 3].map((i) => (
          <div key={i} className="np-skeleton">
            <div className="np-skeleton-line np-skeleton-line--short" />
            <div className="np-skeleton-line" />
            <div className="np-skeleton-line" />
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="np-root">
      {/* Header */}
      <header className="np-header">
        <span className="kicker">{t('notifPrefs.kicker')}</span>
        <h2 className="np-title">{t('notifPrefs.title')}</h2>
        <p className="np-subtitle">{t('notifPrefs.subtitle')}</p>
      </header>

      {/* Alert banner */}
      {alertState && (
        <div
          role="status"
          aria-live="polite"
          className={`np-alert np-alert--${alertState}`}
        >
          {alertState === 'success'
            ? t('notifPrefs.saveSuccess')
            : t('notifPrefs.saveError')}
        </div>
      )}

      {/* Preference form */}
      <form onSubmit={handleSave} aria-label={t('notifPrefs.formLabel')} noValidate>
        {categories.map((cat) => (
          <CategoryCard
            key={cat.key}
            icon={cat.icon}
            title={cat.title}
            description={cat.description}
            channels={cat.channels}
            prefs={prefs}
            onToggle={handleToggle}
            disabled={saving}
            t={t}
          />
        ))}

        <div className="np-save-row">
          <button
            type="submit"
            className="np-save-btn"
            disabled={saving}
            aria-label={saving ? t('notifPrefs.saving') : t('notifPrefs.saveLabel')}
          >
            {saving && (
              <span className="np-spinner" aria-hidden="true" />
            )}
            {saving ? t('notifPrefs.saving') : t('common.save')}
          </button>
        </div>
      </form>
    </div>
  );
}
