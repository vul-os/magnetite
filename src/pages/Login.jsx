import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { getOAuthUrl } from '../api/client';
import AuthForm from '../components/auth/AuthForm';
import OAuthButtons from '../components/auth/OAuthButtons';
import EmailInput from '../components/auth/EmailInput';
import PasswordInput from '../components/auth/PasswordInput';
import SocialProof from '../components/auth/SocialProof';
import './auth.css';

const MagnetiteLogo = () => (
  <div className="auth-logo-container">
    <div className="auth-logo-icon">M</div>
    <span className="auth-logo-text">Magnetite</span>
  </div>
);

export default function Login() {
  const { login } = useAuth();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(null);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await login(email, password);
    } catch (err) {
      setError(err.message);
      setLoading(false);
    }
  };

  const handleOAuth = (provider) => {
    setOauthLoading(provider);
    setError('');
    window.location.href = getOAuthUrl(provider);
  };

  return (
    <div className="auth-page">
      <div className="auth-background">
        <div className="auth-bg-gradient" />
        <div className="auth-bg-glow auth-bg-glow-1" />
        <div className="auth-bg-glow auth-bg-glow-2" />
        <div className="auth-bg-particles" />
      </div>

      <div className="auth-container-center">
        <AuthForm
          logo={<MagnetiteLogo />}
          title="Log In"
          subtitle="Sign in to continue to your account"
          loading={loading}
        >
          <OAuthButtons
            layout="vertical"
            loadingProvider={oauthLoading}
            onProviderClick={handleOAuth}
          />

          <div className="auth-divider">
            <span>or continue with email</span>
          </div>

          <form onSubmit={handleSubmit} className="auth-form">
            {error && (
              <div className="auth-error">
                <span className="auth-error-icon">!</span>
                {error}
              </div>
            )}

            <EmailInput
              value={email}
              onChange={setEmail}
              placeholder="Email address"
            />

            <PasswordInput
              value={password}
              onChange={setPassword}
              placeholder="Password"
              showStrength={false}
            />

            <div className="auth-form-footer">
              <Link to="/forgot-password" className="auth-link auth-link-forgot">
                Forgot password?
              </Link>
            </div>

            <button type="submit" className="auth-submit-btn" disabled={loading}>
              {loading ? (
                <span className="spinner spinner-sm" />
              ) : (
                'Log In'
              )}
            </button>
          </form>

          <div className="auth-switch">
            <span>Don't have an account?</span>
            <Link to="/register" className="auth-switch-link">Sign up</Link>
          </div>

          <SocialProof />
        </AuthForm>
      </div>
    </div>
  );
}