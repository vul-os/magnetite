import { useState, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Button from '../../components/common/Button';
import './admin.css';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

function authFetch(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  return fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });
}

/* Defaults used if the platform settings endpoint returns no data or is unreachable.
 * The /api/platform/settings endpoint (platform.rs) is defined and mounted in main.rs. */
const DEFAULT_SETTINGS = {
  platformName:              'Magnetite',
  platformFee:               15,
  minimumPayout:             50,
  maintenanceMode:           false,
  registrationEnabled:       true,
  emailVerificationRequired: true,
  maxGameSessionFee:         10,
  supportEmail:              'support@magnetite.io',
  maxGamesPerDeveloper:      20,
};

function serverToLocal(d) {
  return {
    platformName:              d.platform_name              ?? DEFAULT_SETTINGS.platformName,
    platformFee:               parseFloat(d.platform_fee   ?? DEFAULT_SETTINGS.platformFee),
    minimumPayout:             parseInt(d.minimum_payout   ?? DEFAULT_SETTINGS.minimumPayout),
    maintenanceMode:           d.maintenance_mode           ?? DEFAULT_SETTINGS.maintenanceMode,
    registrationEnabled:       d.registration_enabled       ?? DEFAULT_SETTINGS.registrationEnabled,
    emailVerificationRequired: d.email_verification_required ?? DEFAULT_SETTINGS.emailVerificationRequired,
    maxGameSessionFee:         parseFloat(d.max_game_session_fee ?? DEFAULT_SETTINGS.maxGameSessionFee),
    supportEmail:              d.support_email              ?? DEFAULT_SETTINGS.supportEmail,
    maxGamesPerDeveloper:      parseInt(d.max_games_per_developer ?? DEFAULT_SETTINGS.maxGamesPerDeveloper),
  };
}

function localToServer(s) {
  return {
    platform_name:               s.platformName,
    platform_fee:                s.platformFee,
    minimum_payout:              s.minimumPayout,
    maintenance_mode:            s.maintenanceMode,
    registration_enabled:        s.registrationEnabled,
    email_verification_required: s.emailVerificationRequired,
    max_game_session_fee:        s.maxGameSessionFee,
    support_email:               s.supportEmail,
    max_games_per_developer:     s.maxGamesPerDeveloper,
  };
}

export default function AdminSettings() {
  const [settings, setSettings]       = useState(DEFAULT_SETTINGS);
  const [saving, setSaving]           = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError]     = useState(null);
  const [loading, setLoading]         = useState(true);
  /* Track whether the settings endpoint is mounted; if not, show an info notice */
  const [endpointAvailable, setEndpointAvailable] = useState(true);

  const fetchSettings = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const res = await authFetch('/api/platform/settings');
      if (res.status === 404 || res.status === 405) {
        /* endpoint not mounted yet — use defaults, show notice */
        setEndpointAvailable(false);
      } else if (res.ok) {
        const d = await res.json();
        setSettings(serverToLocal(d));
        setEndpointAvailable(true);
      }
    } catch {
      /* network error — fall back to defaults silently */
      setEndpointAvailable(false);
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch admin settings from the API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchSettings(); }, [fetchSettings]);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    setSaveError(null);
    setSaveSuccess(false);
    try {
      if (!endpointAvailable) {
        throw new Error('Platform settings API is not reachable. Check that the backend is running and the platform router is mounted.');
      }
      const res = await authFetch('/api/platform/settings', {
        method: 'PUT',
        body: JSON.stringify(localToServer(settings)),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || `Save failed (HTTP ${res.status})`);
      }
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2500);
    } catch (err) {
      setSaveError(err.message);
    } finally {
      setSaving(false);
    }
  };

  const update = (key, val) => setSettings(prev => ({ ...prev, [key]: val }));

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Platform Control</span>
              <h1>Platform Settings</h1>
              <p>Configure global platform options</p>
            </div>
            {saveSuccess && (
              <span className="status-badge active" role="status" aria-live="polite">
                ✓ Saved
              </span>
            )}
          </header>

          {!endpointAvailable && !loading && (
            <div
              className="admin-info-banner"
              role="note"
              style={{
                background: 'var(--color-amber-soft)',
                border: '1px solid var(--color-amber)',
                borderRadius: 'var(--radius)',
                padding: '0.75rem 1rem',
                marginBottom: '1.5rem',
                color: 'var(--color-amber)',
                fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-sm)',
              }}
            >
              ⚠ Platform settings API not reachable.
              Settings shown are defaults. Check that the backend is running.
            </div>
          )}

          {saveError && (
            <div className="admin-error-banner" role="alert" style={{ marginBottom: '1rem' }}>
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {saveError}
            </div>
          )}

          {loading ? (
            <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }} aria-busy="true">
              <span className="spinner" aria-hidden="true" /> Loading settings&hellip;
            </div>
          ) : (
            <form className="admin-settings-form" onSubmit={handleSave} noValidate>

              {/* General */}
              <section className="settings-section">
                <h2>General</h2>
                <div className="form-group">
                  <label htmlFor="platformName">Platform Name</label>
                  <input
                    id="platformName"
                    type="text"
                    value={settings.platformName}
                    onChange={(e) => update('platformName', e.target.value)}
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="supportEmail">Support Email</label>
                  <input
                    id="supportEmail"
                    type="email"
                    value={settings.supportEmail}
                    onChange={(e) => update('supportEmail', e.target.value)}
                  />
                </div>
              </section>

              {/* Fees & Payments */}
              <section className="settings-section">
                <h2>Fees &amp; Payments</h2>
                <div className="form-row">
                  <div className="form-group">
                    <label htmlFor="platformFee">Platform Fee (%)</label>
                    <input
                      id="platformFee"
                      type="number"
                      min="0"
                      max="100"
                      step="0.1"
                      value={settings.platformFee}
                      onChange={(e) => update('platformFee', parseFloat(e.target.value))}
                    />
                    <span className="helper-text">Percentage retained per transaction</span>
                  </div>
                  <div className="form-group">
                    <label htmlFor="minimumPayout">Minimum Payout ($)</label>
                    <input
                      id="minimumPayout"
                      type="number"
                      min="0"
                      step="1"
                      value={settings.minimumPayout}
                      onChange={(e) => update('minimumPayout', parseInt(e.target.value))}
                    />
                    <span className="helper-text">Minimum balance for payout requests</span>
                  </div>
                </div>
                <div className="form-group">
                  <label htmlFor="maxGameSessionFee">Max Session Fee ($)</label>
                  <input
                    id="maxGameSessionFee"
                    type="number"
                    min="0"
                    step="0.01"
                    max="100"
                    value={settings.maxGameSessionFee}
                    onChange={(e) => update('maxGameSessionFee', parseFloat(e.target.value))}
                  />
                  <span className="helper-text">Cap on per-session fees developers may charge</span>
                </div>
              </section>

              {/* Developer Limits */}
              <section className="settings-section">
                <h2>Developer Limits</h2>
                <div className="form-group">
                  <label htmlFor="maxGamesPerDeveloper">Max Games per Developer</label>
                  <input
                    id="maxGamesPerDeveloper"
                    type="number"
                    min="1"
                    max="100"
                    value={settings.maxGamesPerDeveloper}
                    onChange={(e) => update('maxGamesPerDeveloper', parseInt(e.target.value))}
                  />
                </div>
              </section>

              {/* Platform Controls */}
              <section className="settings-section">
                <h2>Platform Controls</h2>

                <div className="toggle-setting">
                  <div>
                    <span className="toggle-label">Maintenance Mode</span>
                    <p className="toggle-description">When enabled, only admins can access the platform</p>
                  </div>
                  <label className="toggle" aria-label="Maintenance mode">
                    <input
                      type="checkbox"
                      checked={settings.maintenanceMode}
                      onChange={() => update('maintenanceMode', !settings.maintenanceMode)}
                    />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div className="toggle-setting">
                  <div>
                    <span className="toggle-label">Allow New Registrations</span>
                    <p className="toggle-description">Allow new users to create accounts</p>
                  </div>
                  <label className="toggle" aria-label="Allow new registrations">
                    <input
                      type="checkbox"
                      checked={settings.registrationEnabled}
                      onChange={() => update('registrationEnabled', !settings.registrationEnabled)}
                    />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div className="toggle-setting">
                  <div>
                    <span className="toggle-label">Require Email Verification</span>
                    <p className="toggle-description">Users must verify their email before using the platform</p>
                  </div>
                  <label className="toggle" aria-label="Require email verification">
                    <input
                      type="checkbox"
                      checked={settings.emailVerificationRequired}
                      onChange={() => update('emailVerificationRequired', !settings.emailVerificationRequired)}
                    />
                    <span className="toggle-slider" />
                  </label>
                </div>
              </section>

              <div className="form-actions">
                <Button
                  type="submit"
                  variant="primary"
                  loading={saving}
                  disabled={!endpointAvailable}
                >
                  {saving ? 'Saving...' : 'Save Settings'}
                </Button>
              </div>
            </form>
          )}
        </main>
      </div>
    </Layout>
  );
}
