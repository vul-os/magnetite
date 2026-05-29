import { useState } from 'react';
import { GithubIcon, DiscordIcon } from '../../assets/icons';

const GoogleIcon = (props) => (
  <svg viewBox="0 0 24 24" {...props}>
    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
  </svg>
);

const GitlabIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <path d="M22.65 14.39L12 22.13 1.35 14.39a.84.84 0 0 1-.3-.94l1.22-3.78 2.44-7.51A.42.42 0 0 1 4.82 2a.43.43 0 0 1 .58 0 .42.42 0 0 1 .11.18l2.44 7.49h8.1l2.44-7.51A.42.42 0 0 1 18.6 2a.43.43 0 0 1 .58 0 .39.39 0 0 1 .11.18l2.44 7.51L23 13.45a.84.84 0 0 1-.35.94z"/>
  </svg>
);

const PROVIDER_COLORS = {
  google: { bg: '#fff', text: '#3c4043', border: '#dadce0' },
  discord: { bg: '#5865F2', text: '#fff', border: '#5865F2' },
  github: { bg: '#24292e', text: '#fff', border: '#24292e' },
  gitlab: { bg: '#FC6D26', text: '#fff', border: '#FC6D26' },
};

const OAuthProviders = [
  { id: 'google', name: 'Google', Icon: GoogleIcon },
  { id: 'discord', name: 'Discord', Icon: DiscordIcon },
  { id: 'github', name: 'GitHub', Icon: GithubIcon },
  { id: 'gitlab', name: 'GitLab', Icon: GitlabIcon },
];

export default function OAuthButtons({
  layout = 'vertical',
  loadingProvider = null,
  error,
  onProviderClick,
}) {
  const [localLoading, setLocalLoading] = useState(null);

  const handleClick = (providerId) => {
    if (localLoading) return;
    setLocalLoading(providerId);
    onProviderClick(providerId, () => setLocalLoading(null));
  };

  return (
    <div className={`oauth-container ${layout === 'horizontal' ? 'oauth-horizontal' : ''}`}>
      <div className={`oauth-buttons ${layout === 'horizontal' ? 'oauth-buttons-row' : ''}`}>
        {OAuthProviders.map(({ id, name, Icon }) => {
          const colors = PROVIDER_COLORS[id];
          const isLoading = (localLoading || loadingProvider) === id;

          return (
            <button
              key={id}
              type="button"
              className="oauth-btn"
              onClick={() => handleClick(id)}
              disabled={isLoading}
              aria-label={`Continue with ${name}`}
              style={{
                '--provider-bg': colors.bg,
                '--provider-text': colors.text,
                '--provider-border': colors.border,
              }}
            >
              {isLoading ? (
                <span className="spinner spinner-sm" />
              ) : (
                <>
                  <Icon className="oauth-icon" />
                  <span className="oauth-name">{name}</span>
                </>
              )}
            </button>
          );
        })}
      </div>
      {error && <p className="oauth-error">{error}</p>}
    </div>
  );
}
