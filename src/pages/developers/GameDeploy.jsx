import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import Button from '../../components/common/Button';
import Input from '../../components/common/Input';
import Select from '../../components/common/Select';
import Modal from '../../components/common/Modal';
import DeploymentStatus from './DeploymentStatus';
import './GameDeploy.css';

const MOCK_REPOS = [
  { value: 'cosmic-raiders', label: 'cosmic-raiders', description: 'Action space shooter game' },
  { value: 'galaxy-conquest', label: 'galaxy-conquest', description: '4X strategy game' },
  { value: 'neon-drift', label: 'neon-drift', description: 'High-speed racing game' },
  { value: 'dungeon-realms', label: 'dungeon-realms', description: 'Roguelike RPG' },
];

const MOCK_BRANCHES = [
  { value: 'main', label: 'main' },
  { value: 'develop', label: 'develop' },
  { value: 'staging', label: 'staging' },
];

const MOCK_TIER_OPTIONS = [
  { value: 'free', label: 'Free', description: 'No subscription required' },
  { value: 'basic', label: 'Basic', description: 'Basic tier subscribers only' },
  { value: 'pro', label: 'Pro', description: 'Pro tier subscribers only' },
  { value: 'enterprise', label: 'Enterprise', description: 'Enterprise tier only' },
];

const MOCK_DEPLOYMENTS = [
  {
    id: 'deploy-1',
    name: 'Cosmic Raiders - Production',
    status: 'success',
    repo: 'exo/cosmic-raiders',
    branch: 'main',
    commit: 'a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6',
    version: '2.4.1',
    duration: '4m 32s',
    url: 'https://cosmic-raiders.magnetite.games',
    startedAt: new Date(Date.now() - 3600000).toISOString(),
    progress: 100,
  },
  {
    id: 'deploy-2',
    name: 'Cosmic Raiders - Staging',
    status: 'building',
    repo: 'exo/cosmic-raiders',
    branch: 'develop',
    commit: 'b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7',
    version: '2.5.0-beta',
    duration: '2m 15s',
    startedAt: new Date(Date.now() - 135000).toISOString(),
    progress: 68,
  },
];

const MOCK_BUILD_LOGS = `[2024-01-15 10:32:15] Starting build process...
[2024-01-15 10:32:16] Cloning repository from https://github.com/exo/cosmic-raiders
[2024-01-15 10:32:18] Checking out commit a1b2c3d
[2024-01-15 10:32:19] Installing dependencies...
[2024-01-15 10:32:24] \x1b[32m✓\x1b[0m Dependencies installed successfully
[2024-01-15 10:32:25] Running linter...
[2024-01-15 10:32:28] \x1b[32m✓\x1b[0m Linting passed
[2024-01-15 10:32:29] Running type checks...
[2024-01-15 10:32:35] \x1b[32m✓\x1b[0m Type checks passed
[2024-01-15 10:32:36] Building game assets...
[2024-01-15 10:33:02] \x1b[32m✓\x1b[0m Game assets built successfully
[2024-01-15 10:33:03] Running tests...
[2024-01-15 10:33:12] \x1b[32m✓\x1b[0m All 47 tests passed
[2024-01-15 10:33:13] Building Docker image...
[2024-01-15 10:34:02] \x1b[32m✓\x1b[0m Docker image built successfully
[2024-01-15 10:34:03] Pushing image to registry...
[2024-01-15 10:34:28] \x1b[32m✓\x1b[0m Image pushed to registry
[2024-01-15 10:34:29] Deploying to production...
[2024-01-15 10:35:41] \x1b[32m✓\x1b[0m Deployment successful!
[2024-01-15 10:35:42] Game is live at: https://cosmic-raiders.magnetite.games`;

export default function GameDeploy() {
  const [step, setStep] = useState(1);
  const [githubConnected, setGithubConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [selectedRepo, setSelectedRepo] = useState('');
  const [selectedBranch, setSelectedBranch] = useState('');
  const [gameSettings, setGameSettings] = useState({
    title: '',
    description: '',
    tier: 'free',
  });
  const [deploying, setDeploying] = useState(false);
  const [deployments, setDeployments] = useState(MOCK_DEPLOYMENTS);
  const [showWebhookModal, setShowWebhookModal] = useState(false);
  const [webhookSecret, setWebhookSecret] = useState('');

  useEffect(() => {
    if (deployments.some(d => d.status === 'building')) {
      const interval = setInterval(() => {
        setDeployments(prev => prev.map(d => {
          if (d.status === 'building') {
            const newProgress = Math.min(d.progress + Math.random() * 5, 95);
            return { ...d, progress: newProgress };
          }
          return d;
        }));
      }, 2000);
      return () => clearInterval(interval);
    }
  }, [deployments]);

  const handleConnectGithub = async () => {
    setConnecting(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 1500));
      setGithubConnected(true);
      setStep(2);
    } catch {
      console.error('Failed to connect GitHub');
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnectGithub = () => {
    setGithubConnected(false);
    setSelectedRepo('');
    setSelectedBranch('');
    setStep(1);
  };

  const handleSelectRepo = (repoValue) => {
    setSelectedRepo(repoValue);
    const repo = MOCK_REPOS.find(r => r.value === repoValue);
    if (repo) {
      setGameSettings(prev => ({ ...prev, title: repo.label }));
    }
  };

  const handleTriggerDeploy = async () => {
    setDeploying(true);
    const newDeployment = {
      id: `deploy-${Date.now()}`,
      name: `${gameSettings.title} - Production`,
      status: 'pending',
      repo: `exo/${selectedRepo}`,
      branch: selectedBranch,
      commit: '',
      version: '1.0.0',
      duration: '—',
      startedAt: new Date().toISOString(),
      progress: 0,
    };

    setDeployments(prev => [newDeployment, ...prev]);

    await new Promise(resolve => setTimeout(resolve, 1000));
    setDeployments(prev => prev.map(d =>
      d.id === newDeployment.id ? { ...d, status: 'building', progress: 5 } : d
    ));

    setTimeout(() => {
      setDeployments(prev => prev.map(d =>
        d.id === newDeployment.id ? { ...d, status: 'success', progress: 100, url: `https://${selectedRepo}.magnetite.games`, logs: MOCK_BUILD_LOGS } : d
      ));
    }, 8000);

    setDeploying(false);
  };

  const handleRollback = (deployId) => {
    console.log('Rolling back deployment:', deployId);
  };

  const handleCancelBuild = (deployId) => {
    setDeployments(prev => prev.filter(d => d.id !== deployId));
  };

  const generateWebhookSecret = () => {
    const secret = Array.from({ length: 32 }, () =>
      Math.random().toString(36).charAt(2)
    ).join('');
    setWebhookSecret(secret);
  };

  return (
    <Layout>
      <div className="game-deploy">
        <header className="deploy-header">
          <div className="header-content">
            <h1>Game Deployment</h1>
            <p>Connect your repository and deploy your game to the Magnetite platform</p>
          </div>
          <div className="header-actions">
            <Button
              variant="ghost"
              onClick={() => setShowWebhookModal(true)}
              leftIcon={
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71" />
                  <path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71" />
                </svg>
              }
            >
              Webhook Config
            </Button>
          </div>
        </header>

        <div className="deploy-grid">
          <div className="deploy-main">
            <div className="step-indicator">
              <div className={`step ${step >= 1 ? 'active' : ''} ${step > 1 ? 'completed' : ''}`}>
                <span className="step-number">1</span>
                <span className="step-label">Connect GitHub</span>
              </div>
              <div className="step-connector" />
              <div className={`step ${step >= 2 ? 'active' : ''} ${step > 2 ? 'completed' : ''}`}>
                <span className="step-number">2</span>
                <span className="step-label">Select Repo</span>
              </div>
              <div className="step-connector" />
              <div className={`step ${step >= 3 ? 'active' : ''}`}>
                <span className="step-number">3</span>
                <span className="step-label">Configure & Deploy</span>
              </div>
            </div>

            {step === 1 && (
              <div className="deploy-card github-connect-card">
                <h2>Connect GitHub Repository</h2>
                <p className="card-description">
                  Link your GitHub account to access your repositories and enable automatic deployments
                </p>

                {!githubConnected ? (
                  <div className="github-connect-section">
                    <div className="github-icon">
                      <svg width="48" height="48" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                      </svg>
                    </div>
                    <h3>Connect with GitHub</h3>
                    <p>Authenticate with GitHub to access your repositories</p>
                    <Button
                      variant="primary"
                      size="lg"
                      onClick={handleConnectGithub}
                      isLoading={connecting}
                      leftIcon={
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                          <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                        </svg>
                      }
                    >
                      Connect GitHub App
                    </Button>
                  </div>
                ) : (
                  <div className="github-connected-section">
                    <div className="connected-badge">
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                      </svg>
                      <span>Connected</span>
                    </div>
                    <Button variant="secondary" onClick={handleDisconnectGithub}>
                      Disconnect
                    </Button>
                  </div>
                )}

                <div className="permissions-list">
                  <h4>Permissions granted:</h4>
                  <ul>
                    <li>
                      <svg width="16" height="16" viewBox="0 0 20 20" fill="currentColor">
                        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd"/>
                      </svg>
                      Read repository contents
                    </li>
                    <li>
                      <svg width="16" height="16" viewBox="0 0 20 20" fill="currentColor">
                        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd"/>
                      </svg>
                      Read and write commit status
                    </li>
                    <li>
                      <svg width="16" height="16" viewBox="0 0 20 20" fill="currentColor">
                        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd"/>
                      </svg>
                      Access deployment workflows
                    </li>
                  </ul>
                </div>
              </div>
            )}

            {step === 2 && (
              <div className="deploy-card repo-select-card">
                <h2>Select Repository</h2>
                <p className="card-description">
                  Choose the repository and branch you want to deploy
                </p>

                <div className="form-section">
                  <div className="form-group">
                    <label>Repository</label>
                    <Select
                      options={MOCK_REPOS.map(r => ({
                        value: r.value,
                        label: r.label,
                      }))}
                      value={selectedRepo}
                      onChange={handleSelectRepo}
                      placeholder="Select a repository..."
                      isSearchable
                    />
                  </div>

                  <div className="form-group">
                    <label>Branch</label>
                    <Select
                      options={MOCK_BRANCHES}
                      value={selectedBranch}
                      onChange={setSelectedBranch}
                      placeholder="Select a branch..."
                    />
                  </div>

                  <div className="form-actions">
                    <Button variant="ghost" onClick={() => setStep(1)}>
                      Back
                    </Button>
                    <Button
                      variant="primary"
                      onClick={() => setStep(3)}
                      isDisabled={!selectedRepo || !selectedBranch}
                    >
                      Continue
                    </Button>
                  </div>
                </div>
              </div>
            )}

            {step === 3 && (
              <div className="deploy-card config-deploy-card">
                <h2>Configure & Deploy</h2>
                <p className="card-description">
                  Set up your game settings and trigger the deployment pipeline
                </p>

                <div className="form-section">
                  <div className="form-group">
                    <label>Game Title</label>
                    <Input
                      placeholder="My Awesome Game"
                      value={gameSettings.title}
                      onChange={(e) => setGameSettings({ ...gameSettings, title: e.target.value })}
                    />
                  </div>

                  <div className="form-group">
                    <label>Description</label>
                    <textarea
                      className="config-textarea"
                      placeholder="Describe your game..."
                      value={gameSettings.description}
                      onChange={(e) => setGameSettings({ ...gameSettings, description: e.target.value })}
                      rows={3}
                    />
                  </div>

                  <div className="form-group">
                    <label>Tier Requirement</label>
                    <Select
                      options={MOCK_TIER_OPTIONS}
                      value={gameSettings.tier}
                      onChange={(value) => setGameSettings({ ...gameSettings, tier: value })}
                    />
                    <span className="form-hint">
                      Subscribers at this tier or higher can access your game
                    </span>
                  </div>

                  <div className="deployment-summary">
                    <h4>Deployment Summary</h4>
                    <div className="summary-grid">
                      <div className="summary-item">
                        <span className="summary-label">Repository</span>
                        <span className="summary-value">{selectedRepo}</span>
                      </div>
                      <div className="summary-item">
                        <span className="summary-label">Branch</span>
                        <span className="summary-value">{selectedBranch}</span>
                      </div>
                      <div className="summary-item">
                        <span className="summary-label">Access Tier</span>
                        <span className="summary-value">{gameSettings.tier}</span>
                      </div>
                    </div>
                  </div>

                  <div className="form-actions">
                    <Button variant="ghost" onClick={() => setStep(2)}>
                      Back
                    </Button>
                    <Button
                      variant="primary"
                      size="lg"
                      onClick={handleTriggerDeploy}
                      isLoading={deploying}
                      isDisabled={!gameSettings.title}
                      leftIcon={
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z" />
                        </svg>
                      }
                    >
                      {deploying ? 'Deploying...' : 'Deploy Game'}
                    </Button>
                  </div>
                </div>
              </div>
            )}

            {deployments.length > 0 && (
              <div className="deployments-section">
                <h3>Recent Deployments</h3>
                <div className="deployments-list">
                  {deployments.map(deployment => (
                    <DeploymentStatus
                      key={deployment.id}
                      deployment={deployment}
                      onRollback={handleRollback}
                      onCancel={handleCancelBuild}
                    />
                  ))}
                </div>
              </div>
            )}
          </div>

          <div className="deploy-sidebar">
            <div className="sidebar-card">
              <h3>Quick Tips</h3>
              <ul className="tips-list">
                <li>
                  <strong>Main branch</strong> deployments go live immediately
                </li>
                <li>
                  <strong>Webhooks</strong> trigger builds on push events
                </li>
                <li>
                  <strong>Rollbacks</strong> restore the previous version
                </li>
                <li>
                  <strong>Build logs</strong> are retained for 7 days
                </li>
              </ul>
            </div>

            <div className="sidebar-card">
              <h3>Need Help?</h3>
              <p>Check our documentation for CI/CD setup guides</p>
              <Link to="/docs/deployment" className="docs-link">
                View Documentation
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6M15 3h6v6M10 14L21 3" />
                </svg>
              </Link>
            </div>
          </div>
        </div>

        <Modal
          isOpen={showWebhookModal}
          onClose={() => setShowWebhookModal(false)}
          title="Webhook Configuration"
          size="lg"
        >
          <div className="webhook-modal-content">
            <p>
              Configure GitHub webhooks to trigger automatic builds on push events.
            </p>

            <div className="webhook-section">
              <h4>Webhook URL</h4>
              <div className="webhook-url-box">
                <code>https://api.magnetite.games/webhooks/github</code>
                <button
                  className="copy-btn"
                  onClick={() => navigator.clipboard.writeText('https://api.magnetite.games/webhooks/github')}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                  </svg>
                </button>
              </div>
            </div>

            <div className="webhook-section">
              <h4>Webhook Secret</h4>
              <div className="webhook-secret-box">
                <code>{webhookSecret || 'Generate a secret to secure your webhook'}</code>
                <Button variant="secondary" size="sm" onClick={generateWebhookSecret}>
                  Generate
                </Button>
              </div>
            </div>

            <div className="webhook-section">
              <h4>Events to trigger</h4>
              <ul className="events-list">
                <li>
                  <input type="checkbox" id="push" checked readOnly />
                  <label htmlFor="push">Push</label>
                </li>
                <li>
                  <input type="checkbox" id="pr" readOnly />
                  <label htmlFor="pr">Pull requests</label>
                </li>
              </ul>
            </div>

            <div className="webhook-section">
              <h4>Setup Instructions</h4>
              <ol className="setup-instructions">
                <li>Go to your repository on GitHub</li>
                <li>Navigate to Settings → Webhooks → Add webhook</li>
                <li>Paste the webhook URL above</li>
                <li>Set content type to <code>application/json</code></li>
                <li>Generate and copy the secret, then paste it here</li>
                <li>Select "Push" events and save</li>
              </ol>
            </div>
          </div>
        </Modal>
      </div>
    </Layout>
  );
}
