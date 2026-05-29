import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';

export default function ForgotPassword() {
  const [email, setEmail] = useState('');
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await api.auth.forgotPassword(email);
      setSuccess(true);
    } catch (err) {
      setError(err.message || 'Failed to send reset link');
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="auth-container">
        <div className="success-message">
          <h2>Check your email</h2>
          <p>We sent a password reset link to <strong>{email}</strong></p>
          <p className="muted">Check your inbox and follow the instructions to reset your password.</p>
          <Link to="/login" className="btn btn-primary">Back to Login</Link>
        </div>
      </div>
    );
  }

  return (
    <div className="auth-container">
      <h1>Forgot Password</h1>
      <p className="auth-subtitle">Enter your email and we'll send you a reset link</p>
      {error && <div className="error">{error}</div>}

      <form onSubmit={handleSubmit}>
        <input
          type="email"
          placeholder="Email address"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
        />
        <button type="submit" disabled={loading}>
          {loading ? <span className="spinner" /> : 'Send Reset Link'}
        </button>
      </form>

      <p className="auth-footer">
        Remember your password? <Link to="/login">Log in</Link>
      </p>
    </div>
  );
}