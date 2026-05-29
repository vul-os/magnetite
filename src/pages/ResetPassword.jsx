import { useState } from 'react';
import { useSearchParams, Link, useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import './auth.css';

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
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', marginTop: '0.375rem' }}>
      <div style={{ display: 'flex', gap: '0.25rem', flex: 1 }}>
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

export default function ResetPassword() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const token = searchParams.get('token');

  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);

  if (!token) {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
              </div>
              <h1 className="auth-title">Invalid Link</h1>
              <p className="auth-subtitle">This password reset link is invalid or has expired.</p>
            </div>
            <div className="auth-body">
              <Link to="/forgot-password" className="auth-submit-btn" style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                Request New Link
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');

    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }

    if (password !== confirmPassword) {
      setError('Passwords do not match');
      return;
    }

    setLoading(true);
    try {
      await api.auth.resetPassword(token, password);
      setSuccess(true);
      setTimeout(() => navigate('/login'), 3000);
    } catch (err) {
      setError(err.message || 'Failed to reset password. The link may have expired.');
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="auth-page">
        <div className="auth-background">
          <div className="auth-bg-gradient" />
          <div className="auth-bg-glow auth-bg-glow-1" />
        </div>
        <div className="auth-container-center">
          <div className="auth-card">
            <div className="auth-header">
              <div className="auth-logo-container">
                <div className="auth-logo-icon">M</div>
                <span className="auth-logo-text">Magnetite</span>
              </div>
              <h1 className="auth-title">Password Reset!</h1>
              <p className="auth-subtitle">Your password has been successfully reset. Redirecting to login...</p>
            </div>
            <div className="auth-body">
              <Link to="/login" className="auth-submit-btn" style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                Log In Now
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
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
            <h1 className="auth-title">Reset Password</h1>
            <p className="auth-subtitle">Enter your new password below</p>
          </div>

          <div className="auth-body">
            {error && (
              <div className="auth-error">
                <span className="auth-error-icon">!</span>
                {error}
              </div>
            )}

            <form onSubmit={handleSubmit} className="auth-form">
              <div className="input-wrapper">
                <label className="input-label">New Password</label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="input-field"
                  placeholder="New password"
                  required
                  autoComplete="new-password"
                />
                <PasswordStrengthBar password={password} />
              </div>

              <div className="input-wrapper">
                <label className="input-label">Confirm Password</label>
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
                {loading ? <span className="spinner spinner-sm" /> : 'Reset Password'}
              </button>
            </form>
          </div>

          <div className="auth-footer">
            <Link to="/login" className="auth-link">Remember your password? Log in</Link>
          </div>
        </div>
      </div>
    </div>
  );
}
