import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import GamePreview from '../components/GamePreview';
import { api } from '../api/client';
import './GameStudio.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// ── Mock templates — only used when VITE_USE_MOCKS=true ──────────────────────
const MOCK_TEMPLATES = [
  {
    id: 'arena-shooter',
    name: 'Arena Shooter',
    description: 'Top-down multiplayer arena shooter. Authoritative server, interest-filtered views, 60 Hz tick. Includes weapon, health, and respawn systems.',
    tier: 'free',
    tags: ['multiplayer', 'top-down', 'action'],
    player_count: '2–16',
    tick_hz: 60,
    topology: 'SingleRoom',
  },
  {
    id: 'platformer',
    name: 'Platformer',
    description: 'Side-scroller platformer with server-authoritative physics, jump prediction, and collectibles. Client-side prediction built in.',
    tier: 'free',
    tags: ['platformer', 'single-player', 'puzzle'],
    player_count: '1–4',
    tick_hz: 60,
    topology: 'SingleRoom',
  },
  {
    id: 'fps-starter',
    name: 'FPS Starter',
    description: 'First-person shooter with Bevy + rapier3d physics, gamepad support, spatial sharding for large lobbies, and anti-cheat replay verification.',
    tier: 'starter',
    tags: ['fps', 'multiplayer', '3d', 'gamepad'],
    player_count: '2–64',
    tick_hz: 60,
    topology: 'Dedicated',
  },
  {
    id: 'motorsport',
    name: 'Motorsport',
    description: 'Racing game with vehicle physics, analog throttle/brake/steering, lap tracking, and points leaderboard integration.',
    tier: 'starter',
    tags: ['racing', 'gamepad', 'physics', 'leaderboard'],
    player_count: '2–16',
    tick_hz: 60,
    topology: 'Dedicated',
  },
  {
    id: 'strategy',
    name: 'RTS Strategy',
    description: 'Real-time strategy game with a sharded topology for AAA-scale maps, cloud saves, and in-game marketplace for cosmetics.',
    tier: 'advanced',
    tags: ['strategy', 'rts', 'sharded', 'marketplace'],
    player_count: '2–1000',
    tick_hz: 20,
    topology: 'Sharded',
  },
  {
    id: 'blank',
    name: 'Blank Slate',
    description: 'Empty AuthoritativeGame scaffold — implement every method yourself. Maximum control, minimum boilerplate.',
    tier: 'free',
    tags: ['custom', 'blank'],
    player_count: 'Any',
    tick_hz: 60,
    topology: 'SingleRoom',
  },
];

const TIER_CONFIG = {
  free:     { label: 'Free',     color: 'var(--color-success)', bg: 'rgba(61,220,132,0.1)' },
  starter:  { label: 'Starter',  color: 'var(--color-accent)',  bg: 'var(--color-accent-soft)' },
  advanced: { label: 'Advanced', color: 'var(--color-amber)',   bg: 'var(--color-amber-soft)' },
};

const TOPOLOGY_LABELS = {
  SingleRoom: 'Single Room (≤16)',
  Dedicated:  'Dedicated (≤256)',
  Sharded:    'Sharded (AAA)',
};

// ── Step states ──────────────────────────────────────────────────────────────
const STEP_TEMPLATE  = 'template';
const STEP_CONFIGURE = 'configure';
const STEP_RESULT    = 'result';

export default function GameStudio() {
  // ── Step 1: template gallery ─────────────────────────────────────────────
  const [templates, setTemplates]         = useState(USE_MOCKS ? MOCK_TEMPLATES : []);
  const [templatesLoading, setTemplatesLoading] = useState(!USE_MOCKS);
  const [templatesError, setTemplatesError]     = useState(null);
  const [selectedTemplate, setSelectedTemplate] = useState(null);

  // ── Step 2: configure ────────────────────────────────────────────────────
  const [step, setStep]         = useState(STEP_TEMPLATE);
  const [gameName, setGameName] = useState('');
  const [gameDesc, setGameDesc] = useState('');

  // ── Step 3: result ───────────────────────────────────────────────────────
  const [creating, setCreating]     = useState(false);
  const [createError, setCreateError] = useState(null);
  const [result, setResult]         = useState(null);

  // ── Preview ──────────────────────────────────────────────────────────────
  const [showPreview, setShowPreview] = useState(false);

  // ── Load templates ───────────────────────────────────────────────────────
  useEffect(() => {
    if (USE_MOCKS) return;
    let cancelled = false;
    setTemplatesLoading(true);
    setTemplatesError(null);

    api.templates.list()
      .then((data) => {
        if (cancelled) return;
        const list = Array.isArray(data) ? data : (data?.data ?? data?.templates ?? []);
        setTemplates(list.length > 0 ? list : MOCK_TEMPLATES);
      })
      .catch(() => {
        if (cancelled) return;
        // Fall back to built-in templates so the gallery is always usable
        setTemplates(MOCK_TEMPLATES);
        setTemplatesError(null); // not a hard error — mocks cover it
      })
      .finally(() => {
        if (!cancelled) setTemplatesLoading(false);
      });

    return () => { cancelled = true; };
  }, []);

  // ── Handlers ─────────────────────────────────────────────────────────────
  const handleSelectTemplate = (tpl) => {
    setSelectedTemplate(tpl);
    setStep(STEP_CONFIGURE);
    setGameName('');
    setGameDesc('');
    setCreateError(null);
  };

  const handleBack = () => {
    setStep(STEP_TEMPLATE);
    setCreateError(null);
  };

  const handleCreate = async (e) => {
    e.preventDefault();
    if (!gameName.trim() || !selectedTemplate) return;
    setCreating(true);
    setCreateError(null);

    try {
      const body = await api.developer.scaffold({
        name:        gameName.trim(),
        template_id: selectedTemplate.id,
        description: gameDesc.trim() || undefined,
      });
      const res = body?.data ?? body;
      setResult(res);
      setStep(STEP_RESULT);
    } catch (err) {
      setCreateError(err?.message ?? 'Failed to create game');
    } finally {
      setCreating(false);
    }
  };

  const handleStartOver = () => {
    setStep(STEP_TEMPLATE);
    setSelectedTemplate(null);
    setResult(null);
    setCreateError(null);
    setShowPreview(false);
  };

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <Layout>
      <div className="game-studio">
        <header className="studio-header">
          <span className="kicker">// RUST GAME STUDIO</span>
          <h1>Game Studio</h1>
          <p className="studio-subtitle">
            Pick a template, name your game, and get the CLI commands to start building — then deploy to the platform.
          </p>
        </header>

        {/* Step indicator */}
        <div className="studio-steps" aria-label="Steps">
          {[
            { id: STEP_TEMPLATE,  label: 'Choose Template', n: 1 },
            { id: STEP_CONFIGURE, label: 'Configure',        n: 2 },
            { id: STEP_RESULT,    label: 'Get Started',      n: 3 },
          ].map(({ id, label, n }) => {
            const done = (
              (id === STEP_TEMPLATE  && (step === STEP_CONFIGURE || step === STEP_RESULT)) ||
              (id === STEP_CONFIGURE && step === STEP_RESULT)
            );
            const active = step === id;
            return (
              <div key={id} className={`studio-step ${active ? 'active' : ''} ${done ? 'done' : ''}`}>
                <span className="step-num">{done ? '✓' : n}</span>
                <span className="step-label">{label}</span>
              </div>
            );
          })}
        </div>

        {/* Step 1 — Template gallery */}
        {step === STEP_TEMPLATE && (
          <section className="studio-section">
            <div className="studio-section-header">
              <h2>Choose a Template</h2>
              <p className="studio-section-desc">
                Each template implements <code>AuthoritativeGame</code> — pick one and we'll scaffold the Rust crate, ready for <code>magnetite build</code>.
              </p>
            </div>

            {templatesLoading && (
              <div className="studio-loading" aria-live="polite">
                <span className="spinner" aria-hidden="true" />
                Loading templates&hellip;
              </div>
            )}

            {templatesError && (
              <div className="studio-error" role="alert">{templatesError}</div>
            )}

            {!templatesLoading && (
              <div className="template-gallery">
                {templates.map((tpl) => {
                  const tc = TIER_CONFIG[tpl.tier] ?? TIER_CONFIG.free;
                  return (
                    <button
                      key={tpl.id}
                      className={`template-card ${selectedTemplate?.id === tpl.id ? 'selected' : ''}`}
                      onClick={() => handleSelectTemplate(tpl)}
                      aria-pressed={selectedTemplate?.id === tpl.id}
                    >
                      <div className="template-card-top">
                        <div className="template-icon" aria-hidden="true">
                          {tpl.id === 'arena-shooter' ? '⬡' :
                           tpl.id === 'platformer'    ? '▲' :
                           tpl.id === 'fps-starter'   ? '◎' :
                           tpl.id === 'motorsport'    ? '⬬' :
                           tpl.id === 'strategy'      ? '⬟' : '⬢'}
                        </div>
                        <span
                          className="tier-badge"
                          style={{ background: tc.bg, color: tc.color }}
                        >
                          {tc.label}
                        </span>
                      </div>

                      <h3 className="template-name">{tpl.name}</h3>
                      <p className="template-desc">{tpl.description}</p>

                      <div className="template-meta">
                        <span className="meta-item">
                          <span className="meta-label">Players</span>
                          <span className="meta-value">{tpl.player_count ?? '?'}</span>
                        </span>
                        <span className="meta-item">
                          <span className="meta-label">Tick</span>
                          <span className="meta-value">{tpl.tick_hz ?? 60} Hz</span>
                        </span>
                        <span className="meta-item">
                          <span className="meta-label">Topology</span>
                          <span className="meta-value">{TOPOLOGY_LABELS[tpl.topology] ?? tpl.topology ?? '—'}</span>
                        </span>
                      </div>

                      <div className="template-tags">
                        {(tpl.tags ?? []).map((tag) => (
                          <span key={tag} className="tag">{tag}</span>
                        ))}
                      </div>

                      <span className="template-cta">Select template →</span>
                    </button>
                  );
                })}
              </div>
            )}
          </section>
        )}

        {/* Step 2 — Configure */}
        {step === STEP_CONFIGURE && selectedTemplate && (
          <section className="studio-section">
            <div className="studio-section-header">
              <button className="back-btn" onClick={handleBack} aria-label="Back to template gallery">
                ← Templates
              </button>
              <h2>Configure Your Game</h2>
              <p className="studio-section-desc">
                Using template: <strong>{selectedTemplate.name}</strong>
              </p>
            </div>

            <div className="configure-grid">
              <div className="configure-form-card">
                <form onSubmit={handleCreate} className="configure-form">
                  <div className="form-group">
                    <label htmlFor="game-name">Game Name <span aria-hidden="true">*</span></label>
                    <input
                      id="game-name"
                      type="text"
                      placeholder="my-awesome-game"
                      value={gameName}
                      onChange={(e) => setGameName(e.target.value)}
                      required
                      autoFocus
                      pattern="[a-zA-Z0-9_\- ]+"
                      title="Letters, numbers, spaces, hyphens and underscores"
                    />
                    <span className="form-hint">Used as the Cargo crate name (alphanumeric + hyphens)</span>
                  </div>

                  <div className="form-group">
                    <label htmlFor="game-desc">Description</label>
                    <textarea
                      id="game-desc"
                      placeholder="Describe your game…"
                      value={gameDesc}
                      onChange={(e) => setGameDesc(e.target.value)}
                      rows={3}
                    />
                  </div>

                  {createError && (
                    <div className="studio-error" role="alert">{createError}</div>
                  )}

                  <div className="form-actions">
                    <button type="button" className="btn btn-secondary" onClick={handleBack}>
                      Back
                    </button>
                    <button
                      type="submit"
                      className="btn btn-primary"
                      disabled={creating || !gameName.trim()}
                    >
                      {creating ? (
                        <><span className="spinner" aria-hidden="true" /> Creating…</>
                      ) : 'Create Game'}
                    </button>
                  </div>
                </form>
              </div>

              {/* Template summary */}
              <div className="template-summary-card">
                <span className="kicker">// TEMPLATE SUMMARY</span>
                <h3>{selectedTemplate.name}</h3>
                <p>{selectedTemplate.description}</p>
                <div className="summary-grid">
                  <div className="summary-row">
                    <span className="summary-label">Topology</span>
                    <span className="summary-value mono">{TOPOLOGY_LABELS[selectedTemplate.topology] ?? selectedTemplate.topology}</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">Tick rate</span>
                    <span className="summary-value mono">{selectedTemplate.tick_hz ?? 60} Hz</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">Players</span>
                    <span className="summary-value mono">{selectedTemplate.player_count ?? '?'}</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">Tier</span>
                    <span className="summary-value mono">{selectedTemplate.tier}</span>
                  </div>
                </div>
                <div className="template-tags" style={{ marginTop: '1rem' }}>
                  {(selectedTemplate.tags ?? []).map((tag) => (
                    <span key={tag} className="tag">{tag}</span>
                  ))}
                </div>
              </div>
            </div>
          </section>
        )}

        {/* Step 3 — Result + next steps */}
        {step === STEP_RESULT && result && (
          <section className="studio-section">
            <div className="result-header">
              <div className="result-success-icon" aria-hidden="true">✓</div>
              <h2>Game Created!</h2>
              <p>
                <strong>{result.name ?? gameName}</strong> is registered on Magnetite.
                Follow the steps below to start building.
              </p>
            </div>

            <div className="result-grid">
              {/* CLI instructions */}
              <div className="result-card cli-card">
                <span className="kicker">// GET THE CODE</span>
                <h3>CLI Setup</h3>
                <p className="card-desc">Run these commands to scaffold and connect your local repository:</p>

                {result.cli_instructions ? (
                  <pre className="cli-block">{result.cli_instructions}</pre>
                ) : (
                  <pre className="cli-block">{`# Install the Magnetite CLI
cargo install magnetite-cli

# Scaffold a new game crate from the ${selectedTemplate?.name ?? 'selected'} template
magnetite new ${gameName.replace(/\s+/g, '-').toLowerCase()} \\
  --template ${selectedTemplate?.id ?? 'arena-shooter'} \\
  --game-id ${result.game_id ?? '<game-id>'}

# Enter the generated directory
cd ${gameName.replace(/\s+/g, '-').toLowerCase()}

# Build locally (compiles to wasm32-wasip1)
magnetite build

# Run a local authoritative server on ws://localhost:9001
magnetite dev`}</pre>
                )}
              </div>

              {/* Next steps */}
              <div className="result-card next-steps-card">
                <span className="kicker">// NEXT STEPS</span>
                <h3>What&apos;s Next</h3>
                <ol className="next-steps-list">
                  {(result.next_steps ?? [
                    'Run `magnetite new` to scaffold the crate locally',
                    'Implement your game logic in `src/game.rs` — the template has a working starting point',
                    'Run `magnetite dev` to test locally with a real authoritative server',
                    'Connect your GitHub repo in Game Deploy to enable automatic WASM builds',
                    'When ready, `magnetite deploy` pushes your build to the platform (Bucket D CI runner)',
                  ]).map((step, i) => (
                    <li key={i}>{step}</li>
                  ))}
                </ol>

                <div className="result-actions">
                  <a href="/developers/deploy" className="btn btn-primary">
                    Connect GitHub &amp; Deploy
                  </a>
                  <button className="btn btn-secondary" onClick={handleStartOver}>
                    Create Another Game
                  </button>
                </div>
              </div>
            </div>

            {/* Live preview section — wire up when a dev ws_endpoint is available */}
            <div className="preview-section">
              <div className="preview-section-header">
                <span className="kicker">// LIVE PREVIEW</span>
                <h3>Preview in Browser</h3>
                <p>
                  Once you run <code>magnetite dev</code>, enter the WebSocket URL below to preview your game live in the browser using the Magnetite web client.
                </p>
              </div>

              {!showPreview ? (
                <div className="preview-cta">
                  <button
                    className="btn btn-accent"
                    onClick={() => setShowPreview(true)}
                  >
                    Open Preview
                  </button>
                  <span className="preview-hint">Requires a running <code>magnetite dev</code> server</span>
                </div>
              ) : (
                <GamePreview
                  wsEndpoint={null}
                  devMode
                  onClose={() => setShowPreview(false)}
                />
              )}
            </div>
          </section>
        )}
      </div>
    </Layout>
  );
}
