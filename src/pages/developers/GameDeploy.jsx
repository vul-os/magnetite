import { useState, useEffect, useCallback } from 'react';
import { Link } from 'react-router-dom';
import Layout from '../../components/Layout';
import Button from '../../components/common/Button';
import Input from '../../components/common/Input';
import Select from '../../components/common/Select';
import Modal from '../../components/common/Modal';
import DeploymentStatus from './DeploymentStatus';
import { Unavailable, LoadError } from '../../components/state/Unavailable';
import { getOAuthUrl, api } from '../../api/client';
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
const MOCK_REPOS = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { value: 'cosmic-raiders',  label: 'cosmic-raiders',  description: 'Action space shooter game' },
      { value: 'galaxy-conquest', label: 'galaxy-conquest',  description: '4X strategy game'          },
      { value: 'neon-drift',      label: 'neon-drift',       description: 'High-speed racing game'     },
    ]
  : null;

const MOCK_DEPLOYMENTS = import.meta.env.VITE_USE_MOCKS === 'true'
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
  const status = d.status ?? 'queued';
  return {
    id:        d.id,
    game_id:   d.game_id ?? null,
    version_id: d.version_id ?? d.id ?? null,
    name:      d.name ?? d.game_title ?? 'Deployment',
    status,
    repo:      d.repo ?? d.github_repo ?? '',
    branch:    d.branch ?? d.build_branch ?? 'main',
    commit:    d.commit ?? d.commit_sha ?? '',
    version:   d.version ?? '',
    duration:  d.duration ?? '—',
    url:       d.url ?? d.artifact_url ?? null,
    startedAt: d.started_at ?? d.created_at ?? new Date().toISOString(),
    progress:  d.progress ?? (
      status === 'built'    ? 100 :
      status === 'building' ? 60  :
      status === 'queued'   ? 10  : 0
    ),
  };
}

/**
 * A registered `game_versions` row is the real deployable unit on this backend
 * — there is no separate build-job record. `is_live` is the only status the
 * backend actually knows, so that is the only status we claim.
 */
function versionToDeploy(version, game) {
  return normaliseDeploy({
    id:         version.id,
    game_id:    version.game_id ?? game?.id ?? null,
    version_id: version.id,
    name:       game?.title ?? 'Game',
    status:     version.is_live ? 'success' : 'built',
    repo:       game?.github_repo ?? '',
    commit:     version.commit_sha ?? '',
    version:    version.version ?? '',
    created_at: version.created_at,
    progress:   100,
  });
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
  const [deploymentsError, setDeploymentsError] = useState(null);
  const [showWebhookModal, setShowWebhookModal] = useState(false);

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
      // Reacting to the GitHub OAuth callback in the URL (external system).
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setGithubConnected(true);
      setStep(2);
    }
  }, []);

  /* Load repos from backend when connected */
  const loadRepos = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
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

  // Load repos from the API (external system) once GitHub is connected.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (githubConnected) loadRepos();
  }, [githubConnected, loadRepos]);

  /**
   * Load the deployment history.
   *
   * The backend has no build-job collection; the deployable unit is a
   * `game_versions` row, listed per game at
   * GET /developer/games/:id/versions. So: list the developer's games, then
   * ask each for its versions.
   *
   * A failure here is reported, not swallowed — the old code hid every error
   * behind an empty list, which is what made this page look like it worked.
   */
  const loadDeployments = useCallback(async () => {
    setDeploymentsLoading(true);
    setDeploymentsError(null);
    try {
      const body  = await api.developer.games();
      const games = body?.games ?? body?.data ?? body ?? [];
      const list  = Array.isArray(games) ? games : [];

      const perGame = await Promise.all(
        list.map(async (game) => {
          try {
            const vBody = await api.developer.versions(game.id);
            const versions = vBody?.data ?? vBody ?? [];
            return (Array.isArray(versions) ? versions : []).map(v => versionToDeploy(v, game));
          } catch {
            // One game's versions failing must not blank the whole history.
            return [];
          }
        })
      );

      setDeployments(
        perGame.flat().sort((a, b) => new Date(b.startedAt) - new Date(a.startedAt))
      );
    } catch (err) {
      setDeploymentsError(err.message || 'Failed to load deployments');
    } finally {
      setDeploymentsLoading(false);
    }
  }, []);

  // Load the deployment history from the developer API (external system).
  useEffect(() => {
    if (import.meta.env.VITE_USE_MOCKS === 'true') return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadDeployments();
  }, [loadDeployments]);

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

  // ── Build log modal ──────────────────────────────────────────────────────
  // Build logs are NOT implemented on this node: nothing persists CI output and
  // no route serves it. Rather than open an empty console that looks like it is
  // still loading, the modal states the absence.
  const [logModalOpen, setLogModalOpen]     = useState(false);
  const [logModalDeploy, setLogModalDeploy] = useState(null);

  const handleViewLogs = useCallback((deployment) => {
    setLogModalDeploy(deployment);
    setLogModalOpen(true);
  }, []);

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
      const gameId = game.id ?? game.data?.id;

      /* The game record now exists. Re-read the real version history rather
       * than inventing an optimistic "queued" row: until a version is
       * registered there is genuinely nothing deployed, and saying otherwise
       * would be a claim the backend cannot support. */
      if (gameId) await loadDeployments();
    } catch (err) {
      setDeployError(err.message);
    } finally {
      setDeploying(false);
    }
  };

  const [actionError, setActionError] = useState(null);

  /* Promote — PUT /developer/games/:gameId/versions/:versionId/promote */
  const handlePromote = useCallback(async (deployment) => {
    const gameId    = deployment.game_id;
    const versionId = deployment.version_id;
    if (!gameId || !versionId) {
      setActionError('Cannot promote: this entry has no game or version id.');
      return;
    }
    setActionError(null);
    try {
      await api.developer.promote(gameId, versionId);
      await loadDeployments();
    } catch (err) {
      setActionError(`Promote failed: ${err.message}`);
    }
  }, [loadDeployments]);

  /* Rollback — PUT /developer/games/:gameId/versions/:versionId/rollback.
   * The backend rolls back TO a version id, so the version being acted on is
   * the target; there is no free-text version to prompt for. */
  const handleRollback = useCallback(async (deployment) => {
    const gameId    = deployment.game_id;
    const versionId = deployment.version_id;
    if (!gameId || !versionId) {
      setActionError('Cannot roll back: this entry has no game or version id.');
      return;
    }
    setActionError(null);
    try {
      await api.developer.rollback(gameId, versionId);
      await loadDeployments();
    } catch (err) {
      setActionError(`Rollback failed: ${err.message}`);
    }
  }, [loadDeployments]);

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

            <div className="deployments-section">
              <h3>Deployed versions</h3>

              {actionError && (
                <p className="form-error" role="alert">{actionError}</p>
              )}

              {deploymentsLoading ? (
                <div className="deployments-list" aria-busy="true">
                  <span className="sk sk-row" />
                  <span className="sk sk-row" />
                </div>
              ) : deploymentsError ? (
                <LoadError
                  headingLevel={4}
                  title="Could not load deployed versions"
                  detail={deploymentsError}
                  onRetry={loadDeployments}
                >
                  The version history could not be read from this node. This is a
                  failed request, not a missing feature — retrying may work.
                </LoadError>
              ) : deployments.length === 0 ? (
                <div className="state state-empty">
                  <h4 className="state-title">No versions registered yet</h4>
                  <p className="state-body">
                    A version appears here once a build is registered against one
                    of your games. Push to a connected repository, or register a
                    version with the CLI.
                  </p>
                </div>
              ) : (
                <div className="deployments-list">
                  {deployments.map(deployment => (
                    <DeploymentStatus
                      key={deployment.id}
                      deployment={deployment}
                      onRollback={handleRollback}
                      onViewLogs={() => handleViewLogs(deployment)}
                      onPromote={deployment.status === 'built' ? () => handlePromote(deployment) : null}
                    />
                  ))}
                </div>
              )}
            </div>
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
                  <strong>Build logs</strong> are not stored by this node
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
              <Unavailable
                inline
                headingLevel={5}
                title="Secret generation is not built"
                endpoints={['POST /api/v1/github/webhook-secret']}
              >
                This node cannot mint a webhook secret for you. Choose your own
                (any high-entropy string), set it as{' '}
                <code className="mono">GITHUB_WEBHOOK_SECRET</code> in the
                backend environment, and paste the same value into GitHub.
              </Unavailable>
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
                <li>Paste your <code>GITHUB_WEBHOOK_SECRET</code> value into GitHub&rsquo;s Secret field</li>
                <li>Select &ldquo;Push&rdquo; events and save</li>
              </ol>
            </div>
          </div>
        </Modal>

        {/* ── Build Logs Modal ──────────────────────────────────────────── */}
        <Modal
          isOpen={logModalOpen}
          onClose={() => { setLogModalOpen(false); setLogModalDeploy(null); }}
          title={`Build Logs — ${logModalDeploy?.name ?? ''}`}
          size="lg"
        >
          <Unavailable
            headingLevel={3}
            title="Build logs are not kept on this node"
            endpoints={['GET /api/v1/developer/games/:gameId/builds/:buildId/logs']}
          >
            Nothing on this backend stores CI output, so there are no logs to
            show — for this build or any other. The version metadata below is
            everything the node actually knows about it.
          </Unavailable>

          <dl className="deploy-facts">
            {logModalDeploy?.version && (
              <div>
                <dt className="m-sm">Version</dt>
                <dd className="mono">{logModalDeploy.version}</dd>
              </div>
            )}
            {logModalDeploy?.commit && (
              <div>
                <dt className="m-sm">Commit</dt>
                <dd className="mono break-key">{logModalDeploy.commit}</dd>
              </div>
            )}
            <div>
              <dt className="m-sm">Live</dt>
              <dd className="mono">{logModalDeploy?.status === 'success' ? 'yes' : 'no'}</dd>
            </div>
          </dl>
        </Modal>
      </div>
    </Layout>
  );
}
