import { useState, useEffect, useCallback } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import Button from '../../components/common/Button';
import Input from '../../components/common/Input';
import Select from '../../components/common/Select';
import Modal from '../../components/common/Modal';
import DeploymentStatus from './DeploymentStatus';
import { getOAuthUrl } from '../../api/client';
import './GameDeploy.css';

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

/* Mock data — only used when VITE_USE_MOCKS=true */
const MOCK_REPOS = import.meta.env.VITE_USE_MOCKS
  ? [
      { value: 'cosmic-raiders',  label: 'cosmic-raiders',  description: 'Action space shooter game' },
      { value: 'galaxy-conquest', label: 'galaxy-conquest',  description: '4X strategy game'          },
      { value: 'neon-drift',      label: 'neon-drift',       description: 'High-speed racing game'     },
    ]
  : null;

const MOCK_DEPLOYMENTS = import.meta.env.VITE_USE_MOCKS
  ? [
      {
        id:         'deploy-1',
        name:       'Cosmic Raiders - Production',
        status:     'success',
        repo:       'exo/cosmic-raiders',
        branch:     'main',
        commit:     'a1b2c3d4',
        version:    '2.4.1',
        duration:   '4m 32s',
        url:        'https://cosmic-raiders.magnetite.games',
        startedAt:  new Date(Date.now() - 3600000).toISOString(),
        progress:   100,
      },
    ]
  : null;

const MOCK_TIER_OPTIONS = [
  { value: 'free',       label: 'Free',       description: 'No subscription required'   },
  { value: 'basic',      label: 'Basic',      description: 'Basic tier subscribers only' },
  { value: 'pro',        label: 'Pro',        description: 'Pro tier subscribers only'   },
  { value: 'enterprise', label: 'Enterprise', description: 'Enterprise tier only'        },
];

const STATIC_BRANCHES = [
  { value: 'main',    label: 'main'    },
  { value: 'develop', label: 'develop' },
  { value: 'staging', label: 'staging' },
];

function normaliseRepo(r) {
  return {
    value:       r.id ?? r.full_name ?? r.name,
    label:       r.name ?? r.full_name ?? String(r.id),
    description: r.description ?? '',
  };
}

function normaliseDeploy(d) {
  return {
    id:        d.id,
    name:      d.name ?? d.game_title ?? 'Deployment',
    status:    d.status ?? 'pending',
    repo:      d.repo ?? d.github_repo ?? '',
    branch:    d.branch ?? d.build_branch ?? 'main',
    commit:    d.commit ?? d.commit_sha ?? '',
    version:   d.version ?? '',
    duration:  d.duration ?? '—',
    url:       d.url ?? d.artifact_url ?? null,
    startedAt: d.started_at ?? d.created_at ?? new Date().toISOString(),
    progress:  d.progress ?? (d.status === 'success' ? 100 : d.status === 'building' ? 50 : 0),
    logs:      d.logs ?? d.build_log ?? null,
  };
}

export default function GameDeploy() {
  const [step, setStep]               = useState(1);
  const [githubConnected, setGithubConnected] = useState(false);
  const [connecting, setConnecting]   = useState(false);
  const [connectError, setConnectError] = useState(null);
  const [repos, setRepos]             = useState(MOCK_REPOS ?? []);
  const [reposLoading, setReposLoading] = useState(false);
  const [selectedRepo, setSelectedRepo]   = useState('');
  const [selectedBranch, setSelectedBranch] = useState('');
  const [gameSettings, setGameSettings] = useState({ title: '', description: '', tier: 'free' });
  const [deploying, setDeploying]       = useState(false);
  const [deployError, setDeployError]   = useState(null);
  const [deployments, setDeployments]   = useState(MOCK_DEPLOYMENTS ?? []);
  const [deploymentsLoading, setDeploymentsLoading] = useState(!MOCK_DEPLOYMENTS);
  const [showWebhookModal, setShowWebhookModal] = useState(false);
  const [webhookSecret, setWebhookSecret] = useState('');
  const [webhookLoading, setWebhookLoading] = useState(false);

  /* Check existing GitHub installations on mount */
  useEffect(() => {
    async function checkGitHub() {
      try {
        const res = await authFetch('/api/github/installations');
        if (res.ok) {
          const data = await res.json();
          const list = Array.isArray(data) ? data : (data.installations ?? []);
          if (list.length > 0) {
            setGithubConnected(true);
            setStep(2);
          }
        }
      } catch {
        /* not connected */
      }
    }
    checkGitHub();
  }, []);

  /* Handle OAuth callback return */
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('installation_id') || params.get('setup_action') === 'install') {
      window.history.replaceState({}, '', window.location.pathname);
      setGithubConnected(true);
      setStep(2);
    }
  }, []);

  /* Load repos from backend when connected */
  const loadRepos = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) return;
    setReposLoading(true);
    try {
      const res = await authFetch('/api/github/repos');
      if (res.ok) {
        const data = await res.json();
        const raw = Array.isArray(data) ? data : (data.repos ?? data.repositories ?? []);
        setRepos(raw.map(normaliseRepo));
      }
    } catch {
      /* keep empty list */
    } finally {
      setReposLoading(false);
    }
  }, []);

  useEffect(() => {
    if (githubConnected) loadRepos();
  }, [githubConnected, loadRepos]);

  /* Load recent deployments */
  useEffect(() => {
    if (import.meta.env.VITE_USE_MOCKS) return;
    async function loadDeployments() {
      setDeploymentsLoading(true);
      try {
        const res = await authFetch('/api/developer/games');
        if (res.ok) {
          const data = await res.json();
          const raw = data.games ?? data ?? [];
          /* Map games with build statuses to deployment entries */
          const builds = Array.isArray(raw)
            ? raw.flatMap(g =>
                (g.builds ?? []).map(b => normaliseDeploy({ ...b, name: g.title, repo: g.github_repo }))
              )
            : [];
          if (builds.length > 0) setDeployments(builds);
        }
      } catch {
        /* keep empty */
      } finally {
        setDeploymentsLoading(false);
      }
    }
    loadDeployments();
  }, []);

  /* Poll building deployments for status */
  useEffect(() => {
    const building = deployments.filter(d => d.status === 'building' || d.status === 'pending');
    if (building.length === 0) return;
    const interval = setInterval(async () => {
      for (const d of building) {
        try {
          /* Check build status via the build-status endpoint if we have owner/repo */
          const parts = d.repo.split('/');
          if (parts.length === 2) {
            const res = await authFetch(`/api/github/repos/${parts[0]}/${parts[1]}/build-status`);
            if (res.ok) {
              const status = await res.json();
              setDeployments(prev => prev.map(dep =>
                dep.id === d.id
                  ? { ...dep, status: status.status ?? dep.status, progress: status.progress ?? dep.progress }
                  : dep
              ));
            }
          }
        } catch {
          /* ignore poll errors */
        }
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [deployments]);

  const handleConnectGithub = async () => {
    setConnecting(true);
    setConnectError(null);
    try {
      /* Start real GitHub OAuth/App installation flow */
      const oauthUrl = getOAuthUrl('github');
      window.location.href = oauthUrl;
    } catch (err) {
      setConnectError(err.message || 'Failed to start GitHub OAuth');
      setConnecting(false);
    }
  };

  const handleDisconnectGithub = () => {
    setGithubConnected(false);
    setSelectedRepo('');
    setSelectedBranch('');
    setRepos([]);
    setStep(1);
  };

  const handleSelectRepo = (repoValue) => {
    setSelectedRepo(repoValue);
    const repo = repos.find(r => r.value === repoValue);
    if (repo) {
      setGameSettings(prev => ({ ...prev, title: repo.label }));
    }
  };

  const handleTriggerDeploy = async () => {
    setDeploying(true);
    setDeployError(null);
    try {
      /* Create the game record first */
      const res = await authFetch('/api/games', {
        method: 'POST',
        body: JSON.stringify({
          title:        gameSettings.title,
          description:  gameSettings.description,
          github_repo:  selectedRepo,
          branch:       selectedBranch,
          fee_per_session: 0,
        }),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || `Failed to register game (HTTP ${res.status})`);
      }
      const game = await res.json();

      /* The build will be triggered by webhook when the game is registered.
       * Add an optimistic entry showing queued status — actual status comes from the backend. */
      const newDeployment = normaliseDeploy({
        id:         game.id ?? `local-${Date.now()}`,
        name:       gameSettings.title,
        status:     'building',
        repo:       selectedRepo,
        branch:     selectedBranch,
        commit:     '',
        started_at: new Date().toISOString(),
        progress:   5,
      });
      setDeployments(prev => [newDeployment, ...prev]);
    } catch (err) {
      setDeployError(err.message);
    } finally {
      setDeploying(false);
    }
  };

  const handleRollback = async (deployId) => {
    /* TODO: rollback endpoint not yet implemented in the backend.
     * The distribution API has version/artifact management but no rollback trigger.
     * Showing a clear notice rather than a no-op console.log. */
    alert('Rollback is not yet implemented. Manually redeploy an earlier version to roll back.');
    void deployId;
  };

  const handleCancelBuild = (deployId) => {
    setDeployments(prev => prev.filter(d => d.id !== deployId));
  };

  const generateWebhookSecret = async () => {
    setWebhookLoading(true);
    try {
      /* Request a webhook secret from the backend rather than generating client-side */
      const res = await authFetch('/api/github/webhook-secret', { method: 'POST' });
      if (res.ok) {
        const data = await res.json();
        setWebhookSecret(data.secret ?? data.webhook_secret ?? '');
      } else {
        /* Backend endpoint may not exist yet — fall through to disabled state */
        throw new Error('Webhook secret endpoint not available');
      }
    } catch {
      /* TODO: implement POST /api/github/webhook-secret in backend to return a
       * securely-generated secret stored server-side. For now show a notice. */
      setWebhookSecret('');
      alert('Webhook secret generation requires a backend endpoint (not yet implemented). Set GITHUB_WEBHOOK_SECRET in your environment instead.');
    } finally {
      setWebhookLoading(false);
    }
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
                <span className="step-label">Configure &amp; Deploy</span>
              </div>
            </div>

            {step === 1 && (
              <div className="deploy-card github-connect-card">
                <h2>Connect GitHub Repository</h2>
                <p className="card-description">
                  Link your GitHub account to access your repositories and enable automatic deployments
                </p>

                {connectError && (
                  <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
                    <span className="auth-error-icon" aria-hidden="true">!</span>
                    {connectError}
                  </div>
                )}

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
                    {reposLoading ? (
                      <div style={{ color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                        <span className="spinner spinner-sm" aria-hidden="true" /> Loading repositories&hellip;
                      </div>
                    ) : repos.length === 0 ? (
                      <div style={{ color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                        No repositories found. Install the GitHub App on your repositories first.
                      </div>
                    ) : (
                      <Select
                        options={repos.map(r => ({ value: r.value, label: r.label }))}
                        value={selectedRepo}
                        onChange={handleSelectRepo}
                        placeholder="Select a repository..."
                        isSearchable
                      />
                    )}
                  </div>

                  <div className="form-group">
                    <label>Branch</label>
                    <Select
                      options={STATIC_BRANCHES}
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
                <h2>Configure &amp; Deploy</h2>
                <p className="card-description">
                  Set up your game settings and trigger the deployment pipeline
                </p>

                {deployError && (
                  <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
                    <span className="auth-error-icon" aria-hidden="true">!</span>
                    {deployError}
                  </div>
                )}

                <div
                  style={{
                    background: 'var(--color-amber-soft)',
                    border: '1px solid var(--color-amber)',
                    borderRadius: 'var(--radius)',
                    padding: '0.75rem 1rem',
                    marginBottom: '1rem',
                    color: 'var(--color-amber)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: 'var(--text-xs)',
                  }}
                  role="note"
                >
                  WASM build pipeline: deploying registers the game and queues a build.
                  The actual wasm-pack build runs on the CI worker (backend stub — see GAPS.md).
                  Build status will update via webhook when the CI worker reports back.
                </div>

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

            {(deploymentsLoading || deployments.length > 0) && (
              <div className="deployments-section">
                <h3>Recent Deployments</h3>
                {deploymentsLoading ? (
                  <div style={{ padding: '1rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }} aria-busy="true">
                    <span className="spinner spinner-sm" aria-hidden="true" /> Loading deployments&hellip;
                  </div>
                ) : (
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
                )}
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
                  <strong>Build status</strong> updates via GitHub webhook callbacks
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
                <code>{API_BASE}/webhooks/github</code>
                <button
                  className="copy-btn"
                  onClick={() => navigator.clipboard.writeText(`${API_BASE}/webhooks/github`)}
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
                <code>{webhookSecret || 'Click Generate to create a secure secret'}</code>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={generateWebhookSecret}
                  isLoading={webhookLoading}
                >
                  Generate
                </Button>
              </div>
              {webhookSecret && (
                <p style={{ fontSize: 'var(--text-xs)', color: 'var(--color-warning)', fontFamily: 'var(--font-mono)', marginTop: '0.5rem' }}>
                  Copy this secret — set it as GITHUB_WEBHOOK_SECRET in your backend environment.
                </p>
              )}
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
                <li>Navigate to Settings &rarr; Webhooks &rarr; Add webhook</li>
                <li>Paste the webhook URL above</li>
                <li>Set content type to <code>application/json</code></li>
                <li>Generate and copy the secret, then paste it here</li>
                <li>Select &ldquo;Push&rdquo; events and save</li>
              </ol>
            </div>
          </div>
        </Modal>
      </div>
    </Layout>
  );
}
