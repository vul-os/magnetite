import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { api } from '../api/client';

function PasswordStrength({ password }) {
  const [strength, setStrength] = useState(0);

  useEffect(() => {
    let score = 0;
    if (password.length >= 8) score++;
    if (password.length >= 12) score++;
    if (/[A-Z]/.test(password)) score++;
    if (/[0-9]/.test(password)) score++;
    if (/[^A-Za-z0-9]/.test(password)) score++;
    setStrength(score);
  }, [password]);

  if (!password) return null;

  const labels = ['Very Weak', 'Weak', 'Fair', 'Strong', 'Very Strong'];
  const colors = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#10b981'];

  return (
    <div className="password-strength">
      <div className="strength-bar">
        {[1, 2, 3, 4, 5].map((level) => (
          <div
            key={level}
            className="strength-segment"
            style={{
              backgroundColor: strength >= level ? colors[strength - 1] : 'var(--color-border)',
            }}
          />
        ))}
      </div>
      <span className="strength-label" style={{ color: colors[strength - 1] || 'var(--color-text-muted)' }}>
        {labels[strength - 1] || 'Too Short'}
      </span>
    </div>
  );
}

import { useEffect } from 'react';

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
      <div className="auth-container">
        <div className="loading-state">
          <span className="spinner large" />
        </div>
      </div>
    );
  }

  if (!user) {
    return null;
  }

  return (
    <div className="auth-container">
      <h1>Update Password</h1>
      <p className="auth-subtitle">Change your password to keep your account secure</p>
      {error && <div className="error">{error}</div>}
      {success && <div className="success">Password updated successfully!</div>}

      <form onSubmit={handleSubmit}>
        <input
          type="password"
          placeholder="Current password"
          value={currentPassword}
          onChange={(e) => setCurrentPassword(e.target.value)}
          required
        />

        <input
          type="password"
          placeholder="New password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          required
        />
        <PasswordStrength password={newPassword} />

        <input
          type="password"
          placeholder="Confirm new password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          required
        />

        <button type="submit" disabled={loading}>
          {loading ? <span className="spinner" /> : 'Update Password'}
        </button>
      </form>

      <p className="auth-footer">
        <Link to="/settings">Back to Settings</Link>
      </p>
    </div>
  );
}