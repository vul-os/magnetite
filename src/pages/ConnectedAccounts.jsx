import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

const ProviderInfo = {
  google: { name: 'Google', icon: 'G' },
  discord: { name: 'Discord', icon: 'D' },
  github: { name: 'GitHub', icon: 'GH' },
  gitlab: { name: 'GitLab', icon: 'GL' },
};

export default function ConnectedAccounts() {
  const [accounts, setAccounts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [unlinking, setUnlinking] = useState(null);
  const [showConfirm, setShowConfirm] = useState(null);
  const [error, setError] = useState('');

  const loadAccounts = useCallback(async () => {
    try {
      const data = await api.auth.linkedAccounts();
      setAccounts(data);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAccounts();
  }, [loadAccounts]);

  const handleDisconnect = async (accountId) => {
    setUnlinking(accountId);
    try {
      await api.auth.unlinkAccount(accountId);
      setAccounts(accounts.filter(a => a.id !== accountId));
      setShowConfirm(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setUnlinking(null);
    }
  };

  if (loading) {
    return (
      <div className="settings-container">
        <div className="loading-state">
          <div className="spinner"></div>
          <p>Loading connected accounts...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="settings-container">
      <div className="settings-header">
        <h1>Connected Accounts</h1>
        <p>Manage your linked OAuth providers</p>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {accounts.length === 0 ? (
        <div className="empty-state">
          <p>No connected accounts yet</p>
        </div>
      ) : (
        <div className="connected-accounts-list">
          {accounts.map((account) => {
            const provider = ProviderInfo[account.provider] || { name: account.provider, icon: '?' };

            return (
              <div key={account.id} className="connected-account-card">
                <div className="account-header">
                  <div className="account-icon">{provider.icon}</div>
                  <div className="account-title">
                    <h3>{provider.name}</h3>
                  </div>
                </div>
                <div className="account-body">
                  {account.email && (
                    <div className="account-detail">
                      <span className="detail-label">Email:</span>
                      <span className="detail-value">{account.email}</span>
                    </div>
                  )}
                  {account.avatar && (
                    <div className="account-detail">
                      <img src={account.avatar} alt="Avatar" className="account-avatar" loading="lazy" />
                    </div>
                  )}
                  {account.connectedAt && (
                    <div className="account-detail">
                      <span className="detail-label">Connected:</span>
                      <span className="detail-value">
                        {new Date(account.connectedAt).toLocaleDateString()}
                      </span>
                    </div>
                  )}
                </div>
                <div className="account-footer">
                  {showConfirm === account.id ? (
                    <div className="confirm-disconnect">
                      <span>Disconnect this account?</span>
                      <div className="confirm-buttons">
                        <button
                          className="btn-danger"
                          onClick={() => handleDisconnect(account.id)}
                          disabled={unlinking === account.id}
                        >
                          {unlinking === account.id ? <span className="spinner" /> : 'Disconnect'}
                        </button>
                        <button
                          className="btn-secondary"
                          onClick={() => setShowConfirm(null)}
                          disabled={unlinking === account.id}
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button
                      className="btn-danger-outline"
                      onClick={() => setShowConfirm(account.id)}
                    >
                      Disconnect
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      <div className="settings-footer">
        <a href="/settings/linked-accounts" className="link-back">
          Link a new account
        </a>
      </div>
    </div>
  );
}
