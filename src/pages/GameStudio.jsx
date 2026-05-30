import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import { api, getOAuthUrl } from '../api/client';
import './GameStudio.css';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

function authFetch(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  return fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });
}

const CATEGORIES = ['Action', 'Puzzle', 'Racing', 'RPG', 'Strategy', 'Arcade', 'Sports', 'Casual'];

export default function GameStudio() {
  const [githubRepo, setGithubRepo]     = useState('');
  const [githubConnected, setGithubConnected] = useState(false);
  const [connecting, setConnecting]     = useState(false);
  const [connectError, setConnectError] = useState(null);
  const [installations, setInstallations] = useState([]);
  const [formData, setFormData]         = useState({
    title: '',
    description: '',
    category: 'Action',
    price: '',
    thumbnail: '',
    min_players: 1,
    max_players: 4,
  });
  const [deploying, setDeploying]       = useState(false);
  const [deploySuccess, setDeploySuccess] = useState(false);
  const [deployError, setDeployError]   = useState(null);

  /* Check if GitHub App is already installed (any installations available) */
  useEffect(() => {
    async function checkInstallations() {
      try {
        const res = await authFetch('/api/github/installations');
        if (res.ok) {
          const data = await res.json();
          const list = Array.isArray(data) ? data : (data.installations ?? []);
          setInstallations(list);
          if (list.length > 0) {
            setGithubConnected(true);
          }
        }
      } catch {
        /* Not connected — user can trigger the OAuth flow */
      }
    }
    checkInstallations();
  }, []);

  /* Handle OAuth callback: after GitHub App install, GitHub redirects back
   * with ?installation_id=... or ?code=... The user is already logged in so
   * we just reload installations. */
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('installation_id') || params.get('setup_action') === 'install') {
      /* Clean the URL and mark as connected */
      window.history.replaceState({}, '', window.location.pathname);
      authFetch('/api/github/installations')
        .then(res => res.ok ? res.json() : null)
        .then(data => {
          if (data) {
            const list = Array.isArray(data) ? data : (data.installations ?? []);
            setInstallations(list);
            setGithubConnected(list.length > 0);
          }
        })
        .catch(() => {});
    }
  }, []);

  const handleConnectGithub = async () => {
    if (!githubRepo.trim() && !githubConnected) {
      /* Start the GitHub App OAuth/installation flow */
      setConnecting(true);
      setConnectError(null);
      try {
        /* Redirect to GitHub App installation page.
         * The OAuth URL for github provider is /api/oauth/github which starts the flow. */
        const oauthUrl = getOAuthUrl('github');
        window.location.href = oauthUrl;
      } catch (err) {
        setConnectError(err.message || 'Failed to start GitHub OAuth flow');
        setConnecting(false);
      }
      return;
    }

    if (githubRepo.trim()) {
      /* Register a specific repository */
      setConnecting(true);
      setConnectError(null);
      try {
        const [owner, repo] = githubRepo.trim().split('/');
        if (!owner || !repo) throw new Error('Repository must be in owner/repository format');
        const res = await authFetch('/api/github/repos/register', {
          method: 'POST',
          body: JSON.stringify({ owner, repo }),
        });
        if (!res.ok) {
          const err = await res.json().catch(() => ({}));
          throw new Error(err.message || `Registration failed (HTTP ${res.status})`);
        }
        setGithubConnected(true);
      } catch (err) {
        setConnectError(err.message);
      } finally {
        setConnecting(false);
      }
    }
  };

  const handleDisconnectGithub = () => {
    setGithubConnected(false);
    setGithubRepo('');
    setInstallations([]);
  };

  const handleDeploy = async (e) => {
    e.preventDefault();
    setDeploying(true);
    setDeployError(null);
    setDeploySuccess(false);
    try {
      await api.games.create(formData);
      setDeploySuccess(true);
      setTimeout(() => setDeploySuccess(false), 3000);
    } catch (err) {
      setDeployError(err.message || 'Deployment failed');
    } finally {
      setDeploying(false);
    }
  };

  const connectedRepo = installations.length > 0
    ? installations[0]?.full_name ?? installations[0]?.name ?? githubRepo
    : githubRepo;

  return (
    <Layout>
      <div className="game-studio">
        <header className="studio-header">
          <span className="kicker">// RUST GAME STUDIO</span>
          <h1>Game Studio</h1>
          <p className="studio-subtitle">Connect your GitHub repository and configure your Rust game for deployment on Magnetite</p>
        </header>

        <div className="studio-grid">
          <section className="studio-card github-card">
            <span className="kicker">// STEP 1</span>
            <h2>GitHub Integration</h2>
            <p className="card-description">Connect your Rust game repository to enable automatic WASM builds and deployments</p>

            {connectError && (
              <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {connectError}
              </div>
            )}

            {!githubConnected ? (
              <div className="github-connect">
                <div className="input-group">
                  <input
                    type="text"
                    placeholder="owner/repository (optional — or click Connect to install App)"
                    value={githubRepo}
                    onChange={(e) => setGithubRepo(e.target.value)}
                    className="github-input"
                    aria-label="GitHub repository (owner/repository)"
                  />
                </div>
                <button
                  className="btn btn-primary github-btn"
                  onClick={handleConnectGithub}
                  disabled={connecting}
                >
                  {connecting ? (
                    <>
                      <span className="spinner" aria-hidden="true" />
                      Connecting…
                    </>
                  ) : (
                    <>
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                      </svg>
                      Connect GitHub
                    </>
                  )}
                </button>
              </div>
            ) : (
              <div className="github-connected">
                <div className="connected-repo">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                  </svg>
                  <span>{connectedRepo || 'GitHub Connected'}</span>
                  <span className="connected-badge-label">Connected</span>
                </div>
                {installations.length > 1 && (
                  <span style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                    {installations.length} repositories available
                  </span>
                )}
                <button className="btn btn-secondary" onClick={handleDisconnectGithub}>Disconnect</button>
              </div>
            )}

            <div className="github-permissions">
              <h4>Permissions granted</h4>
              <ul>
                <li>Read repository contents (Rust source &amp; Cargo.toml)</li>
                <li>Read commit status</li>
                <li>Read pull request workflows</li>
              </ul>
            </div>
          </section>

          <section className="studio-card config-card">
            <span className="kicker">// STEP 2</span>
            <h2>Game Configuration</h2>
            <p className="card-description">Configure your Rust game&apos;s metadata and playtime pricing</p>

            {deployError && (
              <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
                <span className="auth-error-icon" aria-hidden="true">!</span>
                {deployError}
              </div>
            )}

            <form className="config-form" onSubmit={handleDeploy}>
              <div className="form-group">
                <label htmlFor="game-title">Game Title</label>
                <input
                  id="game-title"
                  type="text"
                  placeholder="My Awesome Rust Game"
                  value={formData.title}
                  onChange={(e) => setFormData({ ...formData, title: e.target.value })}
                  required
                />
              </div>

              <div className="form-group">
                <label htmlFor="game-description">Description</label>
                <textarea
                  id="game-description"
                  placeholder="Describe your game..."
                  value={formData.description}
                  onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                  rows={3}
                  required
                />
              </div>

              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="game-category">Category</label>
                  <select
                    id="game-category"
                    value={formData.category}
                    onChange={(e) => setFormData({ ...formData, category: e.target.value })}
                  >
                    {CATEGORIES.map(cat => (
                      <option key={cat} value={cat}>{cat}</option>
                    ))}
                  </select>
                </div>
                <div className="form-group">
                  <label htmlFor="game-price">Price per Session (USDC)</label>
                  <input
                    id="game-price"
                    type="number"
                    step="0.01"
                    min="0"
                    placeholder="0.50"
                    value={formData.price}
                    onChange={(e) => setFormData({ ...formData, price: e.target.value })}
                    required
                  />
                </div>
              </div>

              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="min-players">Min Players</label>
                  <input
                    id="min-players"
                    type="number"
                    min="1"
                    max="100"
                    value={formData.min_players}
                    onChange={(e) => setFormData({ ...formData, min_players: parseInt(e.target.value) })}
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="max-players">Max Players</label>
                  <input
                    id="max-players"
                    type="number"
                    min="1"
                    max="100"
                    value={formData.max_players}
                    onChange={(e) => setFormData({ ...formData, max_players: parseInt(e.target.value) })}
                  />
                </div>
              </div>

              <div className="form-group">
                <label htmlFor="game-thumbnail">Thumbnail URL</label>
                <input
                  id="game-thumbnail"
                  type="url"
                  placeholder="https://example.com/thumbnail.jpg"
                  value={formData.thumbnail}
                  onChange={(e) => setFormData({ ...formData, thumbnail: e.target.value })}
                />
              </div>

              <button
                type="submit"
                className="btn btn-primary deploy-btn"
                disabled={deploying || !formData.title || !githubConnected}
              >
                {deploying ? (
                  <>
                    <span className="spinner" aria-hidden="true" />
                    Deploying…
                  </>
                ) : deploySuccess ? (
                  <>
                    <svg width="18" height="18" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                      <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd"/>
                    </svg>
                    Deployed!
                  </>
                ) : (
                  'Deploy Game'
                )}
              </button>

              {!githubConnected && (
                <p style={{ marginTop: '0.5rem', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                  Connect GitHub (Step 1) to enable deployment
                </p>
              )}
            </form>
          </section>
        </div>
      </div>
    </Layout>
  );
}
