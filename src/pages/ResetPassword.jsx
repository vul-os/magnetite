import { useState, useEffect } from 'react';
import { useSearchParams, Link, useNavigate } from 'react-router-dom';
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
      <div className="auth-container">
        <div className="error-state">
          <h2>Invalid Link</h2>
          <p>This password reset link is invalid or has expired.</p>
          <Link to="/forgot-password" className="btn btn-primary">Request New Link</Link>
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
      <div className="auth-container">
        <div className="success-message">
          <h2>Password Reset Complete</h2>
          <p>Your password has been successfully reset.</p>
          <p className="muted">Redirecting to login...</p>
          <Link to="/login" className="btn btn-primary">Log In</Link>
        </div>
      </div>
    );
  }

  return (
    <div className="auth-container">
      <h1>Reset Password</h1>
      <p className="auth-subtitle">Enter your new password below</p>
      {error && <div className="error">{error}</div>}

      <form onSubmit={handleSubmit}>
        <div className="password-input-wrapper">
          <input
            type="password"
            placeholder="New password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </div>
        <PasswordStrength password={password} />

        <input
          type="password"
          placeholder="Confirm new password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          required
        />

        <button type="submit" disabled={loading}>
          {loading ? <span className="spinner" /> : 'Reset Password'}
        </button>
      </form>

      <p className="auth-footer">
        Remember your password? <Link to="/login">Log in</Link>
      </p>
    </div>
  );
}