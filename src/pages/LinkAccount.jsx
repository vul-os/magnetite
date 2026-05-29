import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import { getOAuthUrl, api } from '../api/client';

const OAuthProviders = [
  { id: 'google',  name: 'Google',  abbr: 'G',  cls: 'google' },
  { id: 'discord', name: 'Discord', abbr: 'Di', cls: 'discord' },
  { id: 'github',  name: 'GitHub',  abbr: 'GH', cls: 'github' },
  { id: 'gitlab',  name: 'GitLab', abbr: 'GL', cls: 'gitlab' },
];

const USER_KEY = 'magnetite_user';

function navigateToProvider(provider) {
  const destination = encodeURIComponent('/settings/connected-accounts');
  window.location.assign(getOAuthUrl(provider) + `?action=link&destination=${destination}`);
}

export default function LinkAccount() {
  const navigate = useNavigate();
  const [linkedAccounts, setLinkedAccounts] = useState([]);
  const [loading, setLoading]               = useState(true);
  const [linking, setLinking]               = useState(null);
  const [unlinking, setUnlinking]           = useState(null);
  const [showConfirm, setShowConfirm]       = useState(null);
  const [error, setError]                   = useState('');

  const loadLinkedAccounts = useCallback(async () => {
    try {
      const accounts = await api.auth.linkedAccounts();
      setLinkedAccounts(accounts);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const userData = localStorage.getItem(USER_KEY);
    if (!userData) {
      navigate('/login', { replace: true });
      return;
    }
    loadLinkedAccounts();
  }, [navigate, loadLinkedAccounts]);

  const handleLinkAccount = (provider) => {
    setLinking(provider);
    navigateToProvider(provider);
  };

  const handleUnlinkAccount = async (providerId, accountId) => {
    setUnlinking(accountId);
    try {
      await api.auth.unlinkAccount(accountId);
      setLinkedAccounts(linkedAccounts.filter(a => a.id !== accountId));
      setShowConfirm(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setUnlinking(null);
    }
  };

  const isLinked = (providerId) => linkedAccounts.some(a => a.provider === providerId);

  return (
    <Layout>
      <div className="connected-accounts-page">
        <header className="settings-page-header reveal reveal-1">
          <span className="kicker">// OAUTH PROVIDERS</span>
          <h1 className="settings-page-title">Link Account</h1>
          <p className="settings-page-subtitle">
            Connect OAuth providers to enable password-free sign-in.
          </p>
        </header>

        {error && (
          <div className="auth-error reveal" role="alert" style={{ marginBottom: '1.5rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {error}
          </div>
        )}

        {loading ? (
          <div className="settings-loading">
            <span className="spinner spinner-lg" style={{ color: 'var(--color-accent)' }} aria-label="Loading" />
          </div>
        ) : (
          <div className="connected-accounts-grid reveal reveal-2">
            {OAuthProviders.map((provider) => {
              const linked = isLinked(provider.id);
              const linkedAccount = linkedAccounts.find(a => a.provider === provider.id);

              return (
                <div key={provider.id} className={`provider-card${linked ? ' linked' : ''}`}>
                  <div className={`provider-icon ${provider.cls}`} aria-hidden="true">
                    {provider.abbr}
                  </div>
                  <div className="provider-info">
                    <p className="provider-name">{provider.name}</p>
                    <span className={`provider-status${linked ? ' connected' : ''}`}>
                      {linked ? (linkedAccount?.email || 'Connected') : 'Not connected'}
                    </span>
                  </div>
                  <div className="provider-actions">
                    {linked ? (
                      showConfirm === provider.id ? (
                        <div className="provider-confirm-row">
                          <span className="provider-confirm-label">Disconnect?</span>
                          <button
                            className="provider-disconnect-btn"
                            onClick={() => handleUnlinkAccount(provider.id, linkedAccount.id)}
                            disabled={unlinking === linkedAccount.id}
                          >
                            {unlinking === linkedAccount.id
                              ? <span className="spinner spinner-sm" aria-hidden="true" />
                              : 'Yes'}
                          </button>
                          <button
                            className="settings-action-btn"
                            onClick={() => setShowConfirm(null)}
                            disabled={unlinking === linkedAccount.id}
                            style={{ padding: '0.4rem 0.75rem' }}
                          >
                            No
                          </button>
                        </div>
                      ) : (
                        <button
                          className="provider-disconnect-btn"
                          onClick={() => setShowConfirm(provider.id)}
                        >
                          Disconnect
                        </button>
                      )
                    ) : (
                      <button
                        className="provider-connect-btn"
                        onClick={() => handleLinkAccount(provider.id)}
                        disabled={linking !== null}
                      >
                        {linking === provider.id
                          ? <span className="spinner spinner-sm" aria-hidden="true" />
                          : 'Connect'}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        <div style={{ marginTop: '1.5rem', textAlign: 'right' }} className="reveal reveal-3">
          <button
            className="settings-action-btn"
            onClick={() => navigate('/settings/connected-accounts')}
          >
            View All Connected Accounts →
          </button>
        </div>
      </div>
    </Layout>
  );
}
