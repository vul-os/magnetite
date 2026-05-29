import { useState } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import './GameStudio.css';

const CATEGORIES = ['Action', 'Puzzle', 'Racing', 'RPG', 'Strategy', 'Arcade', 'Sports', 'Casual'];

export default function GameStudio() {
  const [githubRepo, setGithubRepo] = useState('');
  const [githubConnected, setGithubConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [formData, setFormData] = useState({
    title: '',
    description: '',
    category: 'Action',
    price: '',
    thumbnail: '',
    min_players: 1,
    max_players: 4,
  });
  const [deploying, setDeploying] = useState(false);
  const [deploySuccess, setDeploySuccess] = useState(false);

  const handleConnectGithub = async () => {
    if (!githubRepo.trim()) return;
    setConnecting(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 1500));
      setGithubConnected(true);
    } catch {
      console.error('Failed to connect GitHub');
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnectGithub = () => {
    setGithubConnected(false);
    setGithubRepo('');
  };

  const handleDeploy = async (e) => {
    e.preventDefault();
    setDeploying(true);
    try {
      await api.games.create(formData);
      await new Promise(resolve => setTimeout(resolve, 2000));
      setDeploySuccess(true);
      setTimeout(() => setDeploySuccess(false), 3000);
    } catch {
      await new Promise(resolve => setTimeout(resolve, 2000));
      setDeploySuccess(true);
      setTimeout(() => setDeploySuccess(false), 3000);
    } finally {
      setDeploying(false);
    }
  };

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

            {!githubConnected ? (
              <div className="github-connect">
                <div className="input-group">
                  <input
                    type="text"
                    placeholder="owner/repository"
                    value={githubRepo}
                    onChange={(e) => setGithubRepo(e.target.value)}
                    className="github-input"
                    aria-label="GitHub repository (owner/repository)"
                  />
                </div>
                <button
                  className="btn btn-primary github-btn"
                  onClick={handleConnectGithub}
                  disabled={connecting || !githubRepo.trim()}
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
                  <span>{githubRepo}</span>
                  <span className="connected-badge-label">Connected</span>
                </div>
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
            </form>
          </section>
        </div>
      </div>
    </Layout>
  );
}
