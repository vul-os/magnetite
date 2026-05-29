import { useState } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Button from '../../components/common/Button';
import './admin.css';

const INITIAL_SETTINGS = {
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

export default function AdminSettings() {
  const [settings, setSettings]     = useState(INITIAL_SETTINGS);
  const [saving, setSaving]         = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    await new Promise(r => setTimeout(r, 1000));
    setSaving(false);
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2500);
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
              <span
                className="status-badge active"
                role="status"
                aria-live="polite"
              >
                ✓ Saved
              </span>
            )}
          </header>

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
              <h2>Fees & Payments</h2>
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
              <Button type="submit" variant="primary" loading={saving}>
                {saving ? 'Saving...' : 'Save Settings'}
              </Button>
            </div>
          </form>
        </main>
      </div>
    </Layout>
  );
}
