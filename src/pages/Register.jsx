import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { getOAuthUrl } from '../api/client';
import AuthForm from '../components/auth/AuthForm';
import OAuthButtons from '../components/auth/OAuthButtons';
import EmailInput from '../components/auth/EmailInput';
import PasswordInput from '../components/auth/PasswordInput';
import TermsCheckbox from '../components/auth/TermsCheckbox';
import SocialProof from '../components/auth/SocialProof';
import './auth.css';

const MagnetiteLogo = () => (
  <div className="auth-logo-container">
    <div className="auth-logo-icon">M</div>
    <span className="auth-logo-text">Magnetite</span>
  </div>
);

export default function Register() {
  const { register } = useAuth();
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [termsAccepted, setTermsAccepted] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(null);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    if (!termsAccepted) {
      setError('You must accept the terms and conditions');
      return;
    }
    setLoading(true);
    try {
      await register(username, email, password);
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
          title="Sign Up"
          subtitle="Join thousands of teams on Magnetite"
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

            <div className="input-wrapper">
              <label className="input-label">Username</label>
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="input-field"
                placeholder="Choose a username"
                required
                minLength={3}
                maxLength={20}
              />
            </div>

            <EmailInput
              value={email}
              onChange={setEmail}
              placeholder="Email address"
            />

            <PasswordInput
              value={password}
              onChange={setPassword}
              placeholder="Create a password"
              showStrength={true}
              showRequirements={true}
            />

            <TermsCheckbox
              checked={termsAccepted}
              onChange={setTermsAccepted}
              required={true}
            />

            <button
              type="submit"
              className="auth-submit-btn"
              disabled={!termsAccepted || loading}
            >
              {loading ? (
                <span className="spinner spinner-sm" />
              ) : (
                'Sign Up'
              )}
            </button>
          </form>

          <div className="auth-switch">
            <span>Already have an account?</span>
            <Link to="/login" className="auth-switch-link">Log in</Link>
          </div>

          <SocialProof />
        </AuthForm>
      </div>
    </div>
  );
}