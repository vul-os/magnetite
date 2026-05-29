import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { getOAuthUrl, api } from '../api/client';

const OAuthProviders = [
  { id: 'google', name: 'Google', icon: 'G' },
  { id: 'discord', name: 'Discord', icon: 'D' },
  { id: 'github', name: 'GitHub', icon: 'GH' },
  { id: 'gitlab', name: 'GitLab', icon: 'GL' },
];

const USER_KEY = 'magnetite_user';

function navigateToProvider(provider) {
  const destination = encodeURIComponent('/settings/connected-accounts');
  window.location.assign(getOAuthUrl(provider) + `?action=link&destination=${destination}`);
}

export default function LinkAccount() {
  const navigate = useNavigate();
  const [linkedAccounts, setLinkedAccounts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [linking, setLinking] = useState(null);
  const [unlinking, setUnlinking] = useState(null);
  const [showConfirm, setShowConfirm] = useState(null);
  const [error, setError] = useState('');

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

  const isLinked = (providerId) => {
    return linkedAccounts.some(a => a.provider === providerId);
  };

  if (loading) {
    return (
      <div className="settings-container">
        <div className="loading-state">
          <div className="spinner"></div>
          <p>Loading linked accounts...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="settings-container">
      <div className="settings-header">
        <h1>Link Account</h1>
        <p>Connect additional OAuth providers to your account</p>
      </div>

      {error && <div className="error-banner">{error}</div>}

      <div className="linked-accounts-list">
        {OAuthProviders.map((provider) => {
          const linked = isLinked(provider.id);
          const linkedAccount = linkedAccounts.find(a => a.provider === provider.id);

          return (
            <div key={provider.id} className="account-card">
              <div className="account-info">
                <div className="account-icon">{provider.icon}</div>
                <div className="account-details">
                  <h3>{provider.name}</h3>
                  {linked ? (
                    <p className="linked-email">{linkedAccount?.email || 'Connected'}</p>
                  ) : (
                    <p className="not-linked">Not linked</p>
                  )}
                </div>
              </div>
              <div className="account-actions">
                {linked ? (
                  showConfirm === provider.id ? (
                    <div className="confirm-unlink">
                      <span>Unlink this account?</span>
                      <div className="confirm-buttons">
                        <button
                          className="btn-danger"
                          onClick={() => handleUnlinkAccount(provider.id, linkedAccount.id)}
                          disabled={unlinking === linkedAccount.id}
                        >
                          {unlinking === linkedAccount.id ? <span className="spinner" /> : 'Unlink'}
                        </button>
                        <button
                          className="btn-secondary"
                          onClick={() => setShowConfirm(null)}
                          disabled={unlinking === linkedAccount.id}
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button
                      className="btn-danger-outline"
                      onClick={() => setShowConfirm(provider.id)}
                    >
                      Disconnect
                    </button>
                  )
                ) : (
                  <button
                    className="btn-primary"
                    onClick={() => handleLinkAccount(provider.id)}
                    disabled={linking !== null}
                  >
                    {linking === provider.id ? <span className="spinner" /> : 'Connect'}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      <div className="settings-footer">
        <button onClick={() => navigate('/settings/connected-accounts')}>
          View Connected Accounts
        </button>
      </div>
    </div>
  );
}
