import { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { api } from '../api/client';
import './auth.css';

const PASSWORD_LEVELS = [
  { min: 5, label: 'Very Strong', color: 'var(--color-success)' },
  { min: 4, label: 'Strong',      color: 'var(--color-success)' },
  { min: 3, label: 'Fair',        color: 'var(--color-warning)' },
  { min: 2, label: 'Weak',        color: 'var(--color-amber)' },
  { min: 1, label: 'Very Weak',   color: 'var(--color-error)' },
];

function getScore(pw) {
  let s = 0;
  if (pw.length >= 8)            s++;
  if (pw.length >= 12)           s++;
  if (/[A-Z]/.test(pw))         s++;
  if (/[0-9]/.test(pw))         s++;
  if (/[^A-Za-z0-9]/.test(pw))  s++;
  return s;
}

function PasswordStrengthBar({ password }) {
  const score = getScore(password);
  if (!password) return null;
  const level = PASSWORD_LEVELS.find((l) => score >= l.min) || PASSWORD_LEVELS[PASSWORD_LEVELS.length - 1];
  return (
    <div className="password-strength">
      <div className="strength-bars">
        {[1, 2, 3, 4, 5].map((seg) => (
          <div
            key={seg}
            className="strength-bar"
            style={{ backgroundColor: score >= seg ? level.color : undefined }}
          />
        ))}
      </div>
      <span className="strength-label" style={{ color: level.color }}>{level.label}</span>
    </div>
  );
}

export default function UpdatePassword() {
  const { user, isLoading: authLoading } = useAuth();
  const navigate = useNavigate();

  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    if (!authLoading && !user) {
      navigate('/login');
    }
  }, [user, authLoading, navigate]);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setSuccess(false);

    if (!currentPassword) {
      setError('Current password is required');
      return;
    }
    if (newPassword.length < 8) {
      setError('New password must be at least 8 characters');
      return;
    }
    if (newPassword !== confirmPassword) {
      setError('New passwords do not match');
      return;
    }

    setLoading(true);
    try {
      await api.auth.updatePassword(currentPassword, newPassword);
      setSuccess(true);
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
    } catch (err) {
      setError(err.message || 'Failed to update password');
    } finally {
      setLoading(false);
    }
  };

  if (authLoading) {
    return (
      <div className="auth-page">
        <div className="auth-background" aria-hidden="true">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)' }} aria-label="Loading" />
        </div>
      </div>
    );
  }

  if (!user) return null;

  return (
    <div className="auth-split">
      {/* Hero */}
      <div className="auth-hero">
        <div className="auth-hero-glow" aria-hidden="true" />
        <div className="auth-hero-glow-amber" aria-hidden="true" />
        <div className="auth-hero-grain" aria-hidden="true" />
        <div className="auth-hero-content">
          <Link to="/" className="auth-hero-logo">
            <div className="auth-hero-logo-mark">M</div>
            <span className="auth-hero-logo-name">Magnetite</span>
          </Link>
          <div className="auth-hero-pitch reveal reveal-2">
            <span className="auth-hero-kicker">// ACCOUNT SECURITY</span>
            <h1 className="auth-hero-heading">
              Keep your account<br />
              <em>locked down.</em>
            </h1>
            <p className="auth-hero-body">
              Use a strong, unique password that you don&apos;t use on other sites.
              Combine it with 2FA for maximum protection.
            </p>
          </div>
          <div className="auth-hero-features reveal reveal-3">
            {[
              'Minimum 8 characters recommended',
              'Mix upper, lower, numbers and symbols',
              'Enable 2FA in Security settings',
            ].map((f) => (
              <div key={f} className="auth-hero-feature">
                <span className="auth-hero-feature-dot" aria-hidden="true" />
                {f}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Form panel */}
      <div className="auth-form-panel">
        <div className="auth-form-inner">
          <div className="auth-panel-header reveal reveal-1">
            <span className="auth-kicker">// UPDATE PASSWORD</span>
            <h1 className="auth-title">Change password</h1>
            <p className="auth-subtitle">Update your password to keep your account secure</p>
          </div>

          {success && (
            <div className="auth-success reveal" role="status">
              <span className="auth-success-icon" aria-hidden="true">✓</span>
              Password updated successfully!
            </div>
          )}

          <form onSubmit={handleSubmit} className="auth-form reveal reveal-2">
            {error && (
              <div className="auth-error" role="alert">
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {error}
              </div>
            )}

            <div className="input-wrapper">
              <label className="input-label" htmlFor="cur-pw">Current Password</label>
              <input
                id="cur-pw"
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className="input-field"
                placeholder="Enter current password"
                required
                autoComplete="current-password"
              />
            </div>

            <div className="input-wrapper">
              <label className="input-label" htmlFor="upd-new-pw">New Password</label>
              <input
                id="upd-new-pw"
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="input-field"
                placeholder="Enter new password"
                required
                autoComplete="new-password"
              />
              <PasswordStrengthBar password={newPassword} />
            </div>

            <div className="input-wrapper">
              <label className="input-label" htmlFor="upd-conf-pw">Confirm New Password</label>
              <input
                id="upd-conf-pw"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="input-field"
                placeholder="Confirm new password"
                required
                autoComplete="new-password"
              />
            </div>

            <button type="submit" className="auth-submit-btn" disabled={loading}>
              {loading
                ? <span className="spinner spinner-sm" aria-hidden="true" />
                : 'Update Password'}
            </button>
          </form>

          <div className="auth-footer reveal reveal-3">
            <Link to="/settings" className="auth-link">← Back to Settings</Link>
          </div>
        </div>
      </div>
    </div>
  );
}
