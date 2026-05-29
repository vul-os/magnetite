import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';

const ProviderMeta = {
  google:  { name: 'Google',  abbr: 'G',  cls: 'google' },
  discord: { name: 'Discord', abbr: 'Di', cls: 'discord' },
  github:  { name: 'GitHub',  abbr: 'GH', cls: 'github' },
  gitlab:  { name: 'GitLab', abbr: 'GL', cls: 'gitlab' },
};

export default function ConnectedAccounts() {
  const [accounts, setAccounts]       = useState([]);
  const [loading, setLoading]         = useState(true);
  const [unlinking, setUnlinking]     = useState(null);
  const [showConfirm, setShowConfirm] = useState(null);
  const [error, setError]             = useState('');

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

  return (
    <Layout>
      <div className="connected-accounts-page">
        <header className="settings-page-header reveal reveal-1">
          <span className="kicker">// OAUTH</span>
          <h1 className="settings-page-title">Connected Accounts</h1>
          <p className="settings-page-subtitle">
            Manage the OAuth providers linked to your Magnetite account.
          </p>
        </header>

        {error && (
          <div className="auth-error reveal" role="alert" style={{ marginBottom: '1.5rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {error}
          </div>
        )}

        {loading ? (
          <div className="settings-loading" style={{ minHeight: 160 }}>
            <span
              className="spinner spinner-lg"
              style={{ color: 'var(--color-accent)' }}
              aria-label="Loading connected accounts"
            />
          </div>
        ) : accounts.length === 0 ? (
          <div className="settings-section reveal reveal-2" style={{ textAlign: 'center', padding: '3rem 2rem' }}>
            <div style={{ fontSize: '2rem', marginBottom: '1rem', color: 'var(--color-text-muted)' }} aria-hidden="true">⬡</div>
            <h3 className="settings-section-title" style={{ marginBottom: '0.5rem' }}>No connected accounts</h3>
            <p style={{ fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm)', color: 'var(--color-text-muted)', margin: '0 0 1.5rem' }}>
              Link a provider to sign in without a password.
            </p>
            <a href="/settings/linked-accounts" className="settings-save-btn" style={{ textDecoration: 'none', display: 'inline-flex', marginTop: 0 }}>
              Link Account
            </a>
          </div>
        ) : (
          <div className="connected-accounts-grid reveal reveal-2">
            {accounts.map((account) => {
              const prov = ProviderMeta[account.provider] || { name: account.provider, abbr: '?', cls: '' };
              return (
                <div key={account.id} className="provider-card linked">
                  <div className={`provider-icon ${prov.cls}`} aria-hidden="true">
                    {prov.abbr}
                  </div>
                  <div className="provider-info">
                    <p className="provider-name">{prov.name}</p>
                    {account.email && (
                      <span className="provider-status connected">{account.email}</span>
                    )}
                    {account.connectedAt && (
                      <span className="provider-status" style={{ marginTop: 2 }}>
                        Connected {new Date(account.connectedAt).toLocaleDateString()}
                      </span>
                    )}
                  </div>
                  <div className="provider-actions">
                    {showConfirm === account.id ? (
                      <div className="provider-confirm-row">
                        <span className="provider-confirm-label">Disconnect?</span>
                        <button
                          className="provider-disconnect-btn"
                          onClick={() => handleDisconnect(account.id)}
                          disabled={unlinking === account.id}
                        >
                          {unlinking === account.id
                            ? <span className="spinner spinner-sm" aria-hidden="true" />
                            : 'Yes'}
                        </button>
                        <button
                          className="settings-action-btn"
                          onClick={() => setShowConfirm(null)}
                          disabled={unlinking === account.id}
                          style={{ padding: '0.4rem 0.75rem' }}
                        >
                          No
                        </button>
                      </div>
                    ) : (
                      <button
                        className="provider-disconnect-btn"
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

        <div className="settings-section reveal reveal-3" style={{ marginTop: '1.5rem' }}>
          <h2 className="settings-section-title">Link a New Account</h2>
          <p className="settings-section-desc">
            Connect additional OAuth providers to sign in faster.
          </p>
          <a href="/settings/linked-accounts" className="settings-save-btn" style={{ textDecoration: 'none', display: 'inline-flex', margin: 0 }}>
            Manage Links
          </a>
        </div>
      </div>
    </Layout>
  );
}
