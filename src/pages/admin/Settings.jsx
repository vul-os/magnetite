import { useState } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Button from '../../components/common/Button';

const MOCK_SETTINGS = {
  platformName: 'Magnetite',
  platformFee: 5,
  minimumPayout: 50,
  maintenanceMode: false,
  registrationEnabled: true,
  emailVerificationRequired: true,
  maxGameSessionFee: 10,
  supportEmail: 'support@magnetite.io',
  maxGamesPerDeveloper: 20,
};

export default function AdminSettings() {
  const [settings, setSettings] = useState(MOCK_SETTINGS);
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    await new Promise(r => setTimeout(r, 1000));
    setSaving(false);
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2000);
  };

  const handleToggle = (key) => {
    setSettings(prev => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <Layout>
      <div className="admin-dashboard">
        <AdminSidebar />
        <main className="admin-main">
          <header className="admin-header">
            <h1>Platform Settings</h1>
            <p>Configure platform-wide settings and options</p>
          </header>

          <form className="admin-settings-form" onSubmit={handleSave}>
            <section className="settings-section">
              <h2>General</h2>
              <div className="form-group">
                <label>Platform Name</label>
                <input
                  type="text"
                  value={settings.platformName}
                  onChange={(e) => setSettings({ ...settings, platformName: e.target.value })}
                />
              </div>
              <div className="form-group">
                <label>Support Email</label>
                <input
                  type="email"
                  value={settings.supportEmail}
                  onChange={(e) => setSettings({ ...settings, supportEmail: e.target.value })}
                />
              </div>
            </section>

            <section className="settings-section">
              <h2>Fees & Payments</h2>
              <div className="form-row">
                <div className="form-group">
                  <label>Platform Fee (%)</label>
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="0.1"
                    value={settings.platformFee}
                    onChange={(e) => setSettings({ ...settings, platformFee: parseFloat(e.target.value) })}
                  />
                  <span className="helper-text">Percentage taken from each transaction</span>
                </div>
                <div className="form-group">
                  <label>Minimum Payout ($)</label>
                  <input
                    type="number"
                    min="0"
                    step="1"
                    value={settings.minimumPayout}
                    onChange={(e) => setSettings({ ...settings, minimumPayout: parseInt(e.target.value) })}
                  />
                  <span className="helper-text">Minimum amount for payout requests</span>
                </div>
              </div>
              <div className="form-group">
                <label>Maximum Game Session Fee ($)</label>
                <input
                  type="number"
                  min="0"
                  step="0.01"
                  max="100"
                  value={settings.maxGameSessionFee}
                  onChange={(e) => setSettings({ ...settings, maxGameSessionFee: parseFloat(e.target.value) })}
                />
                <span className="helper-text">Maximum fee developers can charge per session</span>
              </div>
            </section>

            <section className="settings-section">
              <h2>Developer Limits</h2>
              <div className="form-group">
                <label>Max Games Per Developer</label>
                <input
                  type="number"
                  min="1"
                  max="100"
                  value={settings.maxGamesPerDeveloper}
                  onChange={(e) => setSettings({ ...settings, maxGamesPerDeveloper: parseInt(e.target.value) })}
                />
              </div>
            </section>

            <section className="settings-section">
              <h2>Platform Controls</h2>
              <div className="toggle-setting">
                <div>
                  <span className="toggle-label">Maintenance Mode</span>
                  <p className="toggle-description">When enabled, only admins can access the platform</p>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.maintenanceMode}
                    onChange={() => handleToggle('maintenanceMode')}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
              <div className="toggle-setting">
                <div>
                  <span className="toggle-label">Allow New Registrations</span>
                  <p className="toggle-description">Allow new users to create accounts</p>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.registrationEnabled}
                    onChange={() => handleToggle('registrationEnabled')}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
              <div className="toggle-setting">
                <div>
                  <span className="toggle-label">Require Email Verification</span>
                  <p className="toggle-description">Users must verify their email before using the platform</p>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.emailVerificationRequired}
                    onChange={() => handleToggle('emailVerificationRequired')}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </section>

            <div className="form-actions">
              <Button type="submit" variant="primary" loading={saving}>
                {saving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save Settings'}
              </Button>
            </div>
          </form>
        </main>
      </div>
    </Layout>
  );
}
