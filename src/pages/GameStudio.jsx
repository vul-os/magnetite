import { useState, useEffect, lazy, Suspense } from 'react';
import Layout from '../components/Layout';
import GamePreview from '../components/GamePreview';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './GameStudio.css';

// Lazy-load CodeEditor — Monaco is large; keep it out of the main bundle.
// The dynamic import means Vite/Rollup will split Monaco into its own chunk.
const CodeEditor = lazy(() => import('../components/CodeEditor'));

// ── SVG template preview art ──────────────────────────────────────────────────
// Each function returns a unique geometric composition for the template card.

function ArenaShooterArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <circle cx="60" cy="40" r="34" stroke="rgba(123, 97, 255,0.18)" strokeWidth="1" />
      <circle cx="60" cy="40" r="24" stroke="rgba(123, 97, 255,0.28)" strokeWidth="1" />
      <circle cx="60" cy="40" r="3" fill="rgba(123, 97, 255,0.6)" />
      {/* Players at cardinal positions */}
      <circle cx="60" cy="8"  r="4" fill="#7b61ff" opacity="0.85" />
      <circle cx="92" cy="40" r="4" fill="#f5a524" opacity="0.85" />
      <circle cx="28" cy="58" r="4" fill="#5b9dff" opacity="0.85" />
      {/* Crosshair lines */}
      <line x1="56" y1="40" x2="64" y2="40" stroke="rgba(123, 97, 255,0.5)" strokeWidth="1" />
      <line x1="60" y1="36" x2="60" y2="44" stroke="rgba(123, 97, 255,0.5)" strokeWidth="1" />
      {/* Bullet trail */}
      <line x1="60" y1="43" x2="60" y2="8" stroke="rgba(123, 97, 255,0.25)" strokeWidth="0.75" strokeDasharray="3 2" />
    </svg>
  );
}

function PlatformerArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {/* Platforms */}
      <rect x="8"  y="64" width="40" height="4" rx="2" fill="rgba(123, 97, 255,0.3)" />
      <rect x="40" y="48" width="32" height="4" rx="2" fill="rgba(123, 97, 255,0.25)" />
      <rect x="72" y="32" width="40" height="4" rx="2" fill="rgba(123, 97, 255,0.3)" />
      <rect x="32" y="20" width="28" height="4" rx="2" fill="rgba(123, 97, 255,0.2)" />
      {/* Character */}
      <rect x="22" y="55" width="8" height="9" rx="1.5" fill="#7b61ff" opacity="0.9" />
      <circle cx="26" cy="52" r="3.5" fill="#7b61ff" opacity="0.9" />
      {/* Collectibles */}
      <circle cx="54" cy="43" r="2.5" fill="#f5a524" opacity="0.7" />
      <circle cx="86" cy="27" r="2.5" fill="#f5a524" opacity="0.7" />
      <circle cx="44" cy="15" r="2.5" fill="#f5a524" opacity="0.5" />
      {/* Jump arc */}
      <path d="M26 55 Q44 28 54 43" stroke="rgba(123, 97, 255,0.2)" strokeWidth="1" fill="none" strokeDasharray="3 2" />
    </svg>
  );
}

function FPSArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {/* FPS viewpoint — gun + crosshair */}
      {/* Crosshair */}
      <circle cx="60" cy="36" r="12" stroke="rgba(123, 97, 255,0.35)" strokeWidth="1" />
      <line x1="60" y1="20" x2="60" y2="30" stroke="rgba(123, 97, 255,0.55)" strokeWidth="1.25" />
      <line x1="60" y1="42" x2="60" y2="52" stroke="rgba(123, 97, 255,0.55)" strokeWidth="1.25" />
      <line x1="44" y1="36" x2="54" y2="36" stroke="rgba(123, 97, 255,0.55)" strokeWidth="1.25" />
      <line x1="66" y1="36" x2="76" y2="36" stroke="rgba(123, 97, 255,0.55)" strokeWidth="1.25" />
      <circle cx="60" cy="36" r="2" fill="rgba(123, 97, 255,0.7)" />
      {/* Gun barrel at bottom */}
      <rect x="48" y="62" width="24" height="10" rx="2" fill="rgba(123, 97, 255,0.15)" stroke="rgba(123, 97, 255,0.3)" strokeWidth="0.75" />
      <rect x="55" y="55" width="10" height="10" rx="1" fill="rgba(123, 97, 255,0.2)" stroke="rgba(123, 97, 255,0.3)" strokeWidth="0.75" />
      {/* Enemy silhouette */}
      <ellipse cx="60" cy="30" rx="6" ry="8" fill="rgba(255,84,104,0.15)" stroke="rgba(255,84,104,0.4)" strokeWidth="0.75" />
    </svg>
  );
}

function MotorsportArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {/* Track oval */}
      <ellipse cx="60" cy="42" rx="48" ry="28" stroke="rgba(245,165,36,0.25)" strokeWidth="6" fill="none" />
      <ellipse cx="60" cy="42" rx="34" ry="16" stroke="rgba(245,165,36,0.12)" strokeWidth="1" fill="none" />
      {/* Racing car */}
      <rect x="50" y="12" width="20" height="10" rx="3" fill="#f5a524" opacity="0.9" />
      <rect x="46" y="16" width="28" height="6" rx="1.5" fill="rgba(245,165,36,0.5)" />
      <circle cx="52" cy="24" r="3" fill="rgba(30,30,40,0.9)" stroke="rgba(245,165,36,0.6)" strokeWidth="1" />
      <circle cx="68" cy="24" r="3" fill="rgba(30,30,40,0.9)" stroke="rgba(245,165,36,0.6)" strokeWidth="1" />
      {/* Speed lines */}
      <line x1="10" y1="14" x2="46" y2="14" stroke="rgba(245,165,36,0.3)" strokeWidth="1" strokeDasharray="4 2" />
      <line x1="8"  y1="19" x2="44" y2="19" stroke="rgba(245,165,36,0.2)" strokeWidth="0.75" strokeDasharray="3 3" />
      {/* Finish flag indicator */}
      <rect x="100" y="8" width="12" height="10" rx="1" fill="none" stroke="rgba(123, 97, 255,0.4)" strokeWidth="0.75" />
      <line x1="100" y1="8"  x2="106" y2="8"  stroke="rgba(123, 97, 255,0.4)" strokeWidth="0.75" />
      <line x1="100" y1="11" x2="106" y2="11" stroke="rgba(123, 97, 255,0.4)" strokeWidth="0.75" />
      <line x1="103" y1="8"  x2="103" y2="18" stroke="rgba(123, 97, 255,0.4)" strokeWidth="0.75" />
    </svg>
  );
}

function StrategyArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {/* Hex grid (isometric RTS feel) */}
      {[
        [30,20],[50,20],[70,20],[90,20],
        [20,36],[40,36],[60,36],[80,36],[100,36],
        [30,52],[50,52],[70,52],[90,52],
      ].map(([cx, cy], i) => (
        <polygon
          key={i}
          points={`${cx},${cy-8} ${cx+7},${cy-4} ${cx+7},${cy+4} ${cx},${cy+8} ${cx-7},${cy+4} ${cx-7},${cy-4}`}
          stroke="rgba(123, 97, 255,0.2)"
          strokeWidth="0.75"
          fill={i === 4 ? 'rgba(123, 97, 255,0.12)' : i === 7 ? 'rgba(255,84,104,0.1)' : 'none'}
        />
      ))}
      {/* Units */}
      <circle cx="40" cy="36" r="4" fill="#7b61ff" opacity="0.8" />
      <circle cx="60" cy="36" r="4" fill="#7b61ff" opacity="0.6" />
      <circle cx="80" cy="36" r="4" fill="rgba(255,84,104,0.8)" opacity="0.8" />
      {/* Move arrows */}
      <path d="M44 36 L56 36" stroke="rgba(123, 97, 255,0.45)" strokeWidth="1" markerEnd="url(#arrowhead)" />
    </svg>
  );
}

function BlankArt() {
  return (
    <svg viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      {/* Empty canvas + cursor */}
      <rect x="20" y="12" width="80" height="56" rx="4" stroke="rgba(123, 97, 255,0.18)" strokeWidth="1" strokeDasharray="4 3" />
      {/* Grid dots */}
      {[30,50,70,90].flatMap(x => [20,40,56].map(y => (
        <circle key={`${x}-${y}`} cx={x} cy={y} r="1" fill="rgba(123, 97, 255,0.2)" />
      )))}
      {/* Cursor */}
      <path d="M55 35 L55 55 L60 50 L64 57 L67 56 L63 49 L70 49 Z" fill="rgba(123, 97, 255,0.65)" />
      {/* Plus icon — "add your own" */}
      <line x1="90" y1="22" x2="90" y2="30" stroke="rgba(123, 97, 255,0.4)" strokeWidth="1.5" strokeLinecap="round" />
      <line x1="86" y1="26" x2="94" y2="26" stroke="rgba(123, 97, 255,0.4)" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

const TEMPLATE_ART = {
  'arena-shooter': ArenaShooterArt,
  'platformer':    PlatformerArt,
  'fps-starter':   FPSArt,
  'motorsport':    MotorsportArt,
  'strategy':      StrategyArt,
  'blank':         BlankArt,
};

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
  const { t } = useTranslation();
  // ── Step 1: template gallery ─────────────────────────────────────────────
  const [templates, setTemplates]         = useState(USE_MOCKS ? MOCK_TEMPLATES : []);
  const [templatesLoading, setTemplatesLoading] = useState(!USE_MOCKS);
  const [templatesError, setTemplatesError]     = useState(null);
  const [selectedTemplate, setSelectedTemplate] = useState(null);

  // ── Step 2: configure ────────────────────────────────────────────────────
  const [step, setStep]         = useState(STEP_TEMPLATE);
  const [gameName, setGameName] = useState('');
  const [gameDesc, setGameDesc] = useState('');
  // In-browser code editor — tracks the starter source the dev can edit
  const [editorSource, setEditorSource] = useState('');
  const [showEditor, setShowEditor]     = useState(false);

  // ── Step 3: result ───────────────────────────────────────────────────────
  const [creating, setCreating]     = useState(false);
  const [createError, setCreateError] = useState(null);
  const [result, setResult]         = useState(null);

  // ── Preview ──────────────────────────────────────────────────────────────
  const [showPreview, setShowPreview] = useState(false);
  const [previewEndpoint, setPreviewEndpoint] = useState('');

  // ── Load templates ───────────────────────────────────────────────────────
  // Fetch templates from the API (external system) on mount.
  useEffect(() => {
    if (USE_MOCKS) return;
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
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
    setEditorSource('');
    setShowEditor(false);
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
    setPreviewEndpoint('');
    setEditorSource('');
    setShowEditor(false);
  };

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <Layout>
      <div className="game-studio">
        <header className="studio-header">
          <span className="kicker">{t('game.studioKicker')}</span>
          <h1>{t('game.studioTitle')}</h1>
          <p className="studio-subtitle">
            {t('game.studioSubtitle')}
          </p>
        </header>

        {/* Step indicator */}
        <div className="studio-steps" aria-label={t('game.stepsLabel')}>
          {[
            { id: STEP_TEMPLATE,  label: t('game.stepChooseTemplate'), n: 1 },
            { id: STEP_CONFIGURE, label: t('game.stepConfigure'),       n: 2 },
            { id: STEP_RESULT,    label: t('game.stepGetStarted'),      n: 3 },
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
              <h2>{t('game.chooseTemplateHeading')}</h2>
              <p className="studio-section-desc">
                {t('game.chooseTemplateDesc')}
              </p>
            </div>

            {templatesLoading && (
              <div className="studio-loading" aria-live="polite">
                <span className="spinner" aria-hidden="true" />
                {t('game.loadingTemplates')}
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
                      {/* SVG preview art */}
                      {(() => {
                        const ArtComponent = TEMPLATE_ART[tpl.id];
                        return ArtComponent ? (
                          <div className="template-art" aria-hidden="true">
                            <ArtComponent />
                          </div>
                        ) : null;
                      })()}

                      <div className="template-card-top">
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
                          <span className="meta-label">{t('game.templatePlayers')}</span>
                          <span className="meta-value">{tpl.player_count ?? '?'}</span>
                        </span>
                        <span className="meta-item">
                          <span className="meta-label">{t('game.templateTick')}</span>
                          <span className="meta-value">{tpl.tick_hz ?? 60} Hz</span>
                        </span>
                        <span className="meta-item">
                          <span className="meta-label">{t('game.templateTopology')}</span>
                          <span className="meta-value">{TOPOLOGY_LABELS[tpl.topology] ?? tpl.topology ?? '—'}</span>
                        </span>
                      </div>

                      <div className="template-tags">
                        {(tpl.tags ?? []).map((tag) => (
                          <span key={tag} className="tag">{tag}</span>
                        ))}
                      </div>

                      <span className="template-cta">{t('game.selectTemplate')}</span>
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
              <button className="back-btn" onClick={handleBack} aria-label={t('game.backToTemplates')}>
                {t('game.backLabel')}
              </button>
              <h2>{t('game.configureHeading')}</h2>
              <p className="studio-section-desc">
                {t('game.usingTemplate', { name: selectedTemplate.name })}
              </p>
            </div>

            <div className="configure-grid">
              <div className="configure-form-card">
                <form onSubmit={handleCreate} className="configure-form">
                  <div className="form-group">
                    <label htmlFor="game-name">
                      {t('game.gameNameLabel')} <span aria-hidden="true">*</span>
                      <span className="visually-hidden"> ({t('common.required')})</span>
                    </label>
                    <input
                      id="game-name"
                      type="text"
                      placeholder={t('game.gameNamePlaceholder')}
                      value={gameName}
                      onChange={(e) => setGameName(e.target.value)}
                      required
                      autoFocus
                      pattern="[a-zA-Z0-9_\- ]+"
                      title="Letters, numbers, spaces, hyphens and underscores"
                      aria-required="true"
                      aria-describedby="game-name-hint"
                    />
                    <span id="game-name-hint" className="form-hint">{t('game.gameNameHint')}</span>
                  </div>

                  <div className="form-group">
                    <label htmlFor="game-desc">{t('game.gameDescLabel')}</label>
                    <textarea
                      id="game-desc"
                      placeholder={t('game.gameDescPlaceholder')}
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
                      {t('game.back')}
                    </button>
                    <button
                      type="submit"
                      className="btn btn-primary"
                      disabled={creating || !gameName.trim()}
                      aria-busy={creating}
                    >
                      {creating ? (
                        <><span className="spinner" aria-hidden="true" /> {t('game.creating')}</>
                      ) : t('game.createGame')}
                    </button>
                  </div>
                </form>
              </div>

              {/* Template summary */}
              <div className="template-summary-card">
                <span className="kicker">{t('game.summaryKicker')}</span>
                <h3>{selectedTemplate.name}</h3>
                <p>{selectedTemplate.description}</p>
                <div className="summary-grid">
                  <div className="summary-row">
                    <span className="summary-label">{t('game.summaryTopology')}</span>
                    <span className="summary-value mono">{TOPOLOGY_LABELS[selectedTemplate.topology] ?? selectedTemplate.topology}</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">{t('game.summaryTick')}</span>
                    <span className="summary-value mono">{selectedTemplate.tick_hz ?? 60} Hz</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">{t('game.summaryPlayers')}</span>
                    <span className="summary-value mono">{selectedTemplate.player_count ?? '?'}</span>
                  </div>
                  <div className="summary-row">
                    <span className="summary-label">{t('game.summaryTier')}</span>
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

            {/* ── In-browser code editor (lazy) ──────────────────────────── */}
            <div className="studio-editor-section">
              <div className="studio-editor-toggle-row">
                <div>
                  <span className="kicker">// STARTER SOURCE</span>
                  <p className="studio-section-desc" style={{ marginTop: 0 }}>
                    Preview and edit the starter game source before scaffolding.
                    Changes here are not saved — use <code>magnetite new</code> to generate
                    locally with your chosen template.
                  </p>
                </div>
                <button
                  type="button"
                  className={`btn ${showEditor ? 'btn-secondary' : 'btn-accent'}`}
                  onClick={() => setShowEditor(v => !v)}
                  aria-expanded={showEditor}
                  aria-controls="studio-code-editor-panel"
                >
                  {showEditor ? 'Hide Editor' : 'Open Code Editor'}
                </button>
              </div>

              {showEditor && (
                <div id="studio-code-editor-panel" className="studio-editor-panel">
                  <Suspense
                    fallback={
                      <div className="studio-loading" aria-live="polite">
                        <span className="spinner" aria-hidden="true" />
                        Loading Monaco editor…
                      </div>
                    }
                  >
                    <CodeEditor
                      value={editorSource || undefined}
                      onChange={setEditorSource}
                      language="rust"
                      height="480px"
                      filename={`${gameName ? gameName.replace(/\s+/g, '_').toLowerCase() : 'game'}/src/game.rs`}
                      className="studio-code-editor"
                    />
                  </Suspense>
                  <p className="studio-editor-note">
                    This is an editable preview of <code>src/game.rs</code> from the{' '}
                    <strong>{selectedTemplate.name}</strong> template.
                    Scaffold locally with <code>magnetite new</code> to get the full project structure.
                  </p>
                </div>
              )}
            </div>
          </section>
        )}

        {/* Step 3 — Result + next steps */}
        {step === STEP_RESULT && result && (
          <section className="studio-section">
            <div className="result-header">
              <div className="result-success-icon" aria-hidden="true">✓</div>
              <h2>{t('game.gameCreated')}</h2>
              <p>
                {t('game.gameRegistered', { name: result.name ?? gameName })}
              </p>
            </div>

            <div className="result-grid">
              {/* CLI instructions */}
              <div className="result-card cli-card">
                <span className="kicker">{t('game.cliSetupKicker')}</span>
                <h3>{t('game.cliSetupTitle')}</h3>
                <p className="card-desc">{t('game.cliSetupDesc')}</p>

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
                <span className="kicker">{t('game.nextStepsKicker')}</span>
                <h3>{t('game.nextStepsTitle')}</h3>
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
                    {t('game.deployLink')}
                  </a>
                  <button className="btn btn-secondary" onClick={handleStartOver}>
                    {t('game.createAnother')}
                  </button>
                </div>
              </div>
            </div>

            {/* Live preview section */}
            <div className="preview-section">
              <div className="preview-section-header">
                <span className="kicker">{t('game.previewKicker')}</span>
                <h3>{t('game.previewTitle')}</h3>
                <p>{t('game.previewDesc')}</p>
              </div>

              {!showPreview ? (
                <div className="preview-open-row">
                  <div className="preview-url-input-row">
                    <label htmlFor="preview-ws-url" className="visually-hidden">{t('game.wsUrlLabel')}</label>
                    <input
                      id="preview-ws-url"
                      type="text"
                      className="preview-url-input"
                      placeholder="ws://localhost:9001"
                      value={previewEndpoint}
                      onChange={(e) => setPreviewEndpoint(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && previewEndpoint.trim()) setShowPreview(true);
                      }}
                      aria-label={t('game.wsUrlLabel')}
                    />
                    <button
                      className="btn btn-accent"
                      onClick={() => setShowPreview(true)}
                      disabled={!previewEndpoint.trim()}
                    >
                      {t('game.playInBrowser')}
                    </button>
                  </div>
                  <span className="preview-hint">{t('game.previewHint')}</span>
                </div>
              ) : (
                <GamePreview
                  wsEndpoint={previewEndpoint.trim() || null}
                  devMode={!previewEndpoint.trim()}
                  onClose={() => { setShowPreview(false); setPreviewEndpoint(''); }}
                />
              )}
            </div>
          </section>
        )}
      </div>
    </Layout>
  );
}
