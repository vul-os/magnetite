import { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { api } from '../api/client';

const PASSWORD_LEVELS = [
  { min: 5, label: 'Very Strong', color: '#10b981' },
  { min: 4, label: 'Strong', color: '#22c55e' },
  { min: 3, label: 'Fair', color: '#eab308' },
  { min: 2, label: 'Weak', color: '#f97316' },
  { min: 1, label: 'Very Weak', color: '#ef4444' },
];

function getScore(password) {
  let score = 0;
  if (password.length >= 8) score++;
  if (password.length >= 12) score++;
  if (/[A-Z]/.test(password)) score++;
  if (/[0-9]/.test(password)) score++;
  if (/[^A-Za-z0-9]/.test(password)) score++;
  return score;
}

function PasswordStrengthBar({ password }) {
  const score = getScore(password);

  if (!password) return null;

  const level = PASSWORD_LEVELS.find((l) => score >= l.min) || PASSWORD_LEVELS[PASSWORD_LEVELS.length - 1];

  return (
    <div className="password-strength" style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', marginTop: '0.375rem' }}>
      <div className="strength-bars" style={{ display: 'flex', gap: '0.25rem', flex: 1 }}>
        {[1, 2, 3, 4, 5].map((seg) => (
          <div
            key={seg}
            style={{
              height: 4,
              flex: 1,
              borderRadius: 2,
              backgroundColor: score >= seg ? level.color : 'var(--color-border)',
              transition: 'background-color var(--t) var(--ease-out)',
            }}
          />
        ))}
      </div>
      <span style={{ fontSize: 12, fontWeight: 500, color: level.color, minWidth: 60 }}>
        {level.label}
      </span>
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
        <div className="auth-container-center" style={{ textAlign: 'center' }}>
          <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)' }} />
        </div>
      </div>
    );
  }

  if (!user) {
    return null;
  }

  return (
    <div className="auth-page">
      <div className="auth-background">
        <div className="auth-bg-gradient" />
        <div className="auth-bg-glow auth-bg-glow-1" />
        <div className="auth-bg-glow auth-bg-glow-2" />
      </div>
      <div className="auth-container-center">
        <div className="auth-card">
          <div className="auth-header">
            <div className="auth-logo-container">
              <div className="auth-logo-icon">M</div>
              <span className="auth-logo-text">Magnetite</span>
            </div>
            <h1 className="auth-title">Update Password</h1>
            <p className="auth-subtitle">Change your password to keep your account secure</p>
          </div>

          <div className="auth-body">
            {error && (
              <div className="auth-error">
                <span className="auth-error-icon">!</span>
                {error}
              </div>
            )}
            {success && (
              <div className="auth-error" style={{ background: 'rgba(61,220,132,0.1)', borderColor: 'rgba(61,220,132,0.3)', color: 'var(--color-success)' }}>
                Password updated successfully!
              </div>
            )}

            <form onSubmit={handleSubmit} className="auth-form">
              <div className="input-wrapper">
                <label className="input-label">Current Password</label>
                <input
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
                <label className="input-label">New Password</label>
                <input
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
                <label className="input-label">Confirm New Password</label>
                <input
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
                {loading ? <span className="spinner spinner-sm" /> : 'Update Password'}
              </button>
            </form>
          </div>

          <div className="auth-footer">
            <Link to="/settings" className="auth-link">Back to Settings</Link>
          </div>
        </div>
      </div>
    </div>
  );
}
