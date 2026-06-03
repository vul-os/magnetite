/**
 * Replay.jsx — Replay viewer page.
 *
 * Loads a ReplayLog via GET /api/v1/replays/:id, re-simulates frames
 * at playback speed using the arena-shooter renderer from magnetite-web-client,
 * and exposes scrubber controls via ReplayScrubber.
 *
 * The renderer is canvas-based — no live WebSocket needed.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import { api } from '../api/client';
import ReplayScrubber from '../components/ReplayScrubber';
import './Replay.css';
import '../components/ReplayScrubber.css';

// ── Mock (VITE_USE_MOCKS=true) ────────────────────────────────────────────────
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

function _buildMockReplay() {
  const totalTicks = 300;
  const frames = [];
  const state_hashes = [];
  for (let t = 0; t < totalTicks; t++) {
    frames.push([t, [
      ['player-1', { keys: { forward: t % 20 < 10, backward: false, left: false, right: false, attack: t % 30 === 0 }, mouse: { x: t * 0.5, y: t * 0.3, delta_x: 0.5, delta_y: 0.3, left_button: t % 30 === 0, right_button: false, middle_button: false, scroll: 0 }, sequence: t, timestamp_ms: t * 16 }],
      ['player-2', { keys: { forward: false, backward: t % 20 < 10, left: t % 15 < 7, right: false, attack: t % 45 === 0 }, mouse: { x: 80 - t * 0.3, y: 60 + t * 0.2, delta_x: -0.3, delta_y: 0.2, left_button: t % 45 === 0, right_button: false, middle_button: false, scroll: 0 }, sequence: t, timestamp_ms: t * 16 }],
    ]]);
    state_hashes.push([t, BigInt(t * 13370) % BigInt(2 ** 32)]);
  }
  return {
    id: 'mock-replay-1',
    config: { game_id: 'game-1', tick_hz: 60, max_players: 4, duration_ticks: totalTicks },
    frames,
    state_hashes,
    recorded_at: new Date().toISOString(),
    verdict: 'Clean',
  };
}

// ── Arena replay state machine ────────────────────────────────────────────────
// We drive the arena manually from ReplayLog.frames without a live WS.
// The renderer is imported lazily to avoid SSR issues.

function buildArenaState(frames, tick, _config) {
  // Reconstruct a synthetic ArenaView from cumulative inputs up to `tick`.
  // Since we don't have the real game WASM here, we build a plausible visual
  // state by integrating position from move inputs (forward = +y, right = +x).
  const playerStates = {};
  const arenaHalf = 90;

  for (let t = 0; t <= tick && t < frames.length; t++) {
    const [, playerInputs] = frames[t];
    for (const [pid, inp] of playerInputs) {
      if (!playerStates[pid]) {
        playerStates[pid] = {
          id: pid,
          x: (Object.keys(playerStates).length * 30) - 30,
          y: 0,
          angle: 0,
          hp: 100,
          alive: true,
          score: 0,
          last_shot_tick: 0,
        };
      }
      const ps = playerStates[pid];
      const k = inp.keys || {};
      const spd = 0.6;
      if (k.forward) ps.y -= spd;
      if (k.backward) ps.y += spd;
      if (k.left) ps.x -= spd;
      if (k.right) ps.x += spd;
      // clamp to arena
      ps.x = Math.max(-arenaHalf, Math.min(arenaHalf, ps.x));
      ps.y = Math.max(-arenaHalf, Math.min(arenaHalf, ps.y));
      // angle from mouse
      if (inp.mouse) ps.angle = Math.atan2(inp.mouse.delta_y || 0, inp.mouse.delta_x || 1);
      if (k.attack && t - ps.last_shot_tick > 20) {
        ps.score += 1;
        ps.last_shot_tick = t;
      }
    }
  }

  const players = Object.values(playerStates);
  const [selfState, ...others] = players;

  return {
    self_state: selfState || null,
    other_players: others,
    projectiles: [],
    tick,
  };
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function Replay() {
  const { id } = useParams();
  const canvasRef = useRef(null);
  const renderFnRef = useRef(null);
  const rafRef = useRef(null);
  const intervalRef = useRef(null);

  const [replay, setReplay] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [currentTick, setCurrentTick] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);

  const currentTickRef = useRef(0);
  const playingRef = useRef(false);
  const speedRef = useRef(1);

  // Keep refs in sync
  useEffect(() => { currentTickRef.current = currentTick; }, [currentTick]);
  useEffect(() => { playingRef.current = playing; }, [playing]);
  useEffect(() => { speedRef.current = speed; }, [speed]);

  // Load renderer lazily (it imports canvas utilities)
  useEffect(() => {
    import('../../magnetite-web-client/src/renderer.js')
      .then(({ renderArenaView }) => { renderFnRef.current = renderArenaView; })
      .catch(() => {
        // Fallback minimal renderer if the import fails (e.g. path issues in test env)
        renderFnRef.current = (ctx, state) => {
          ctx.fillStyle = '#07070b';
          ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
          ctx.fillStyle = '#38e1c8';
          ctx.font = '14px monospace';
          ctx.fillText(`TICK ${state.tick ?? 0}`, 12, 24);
        };
      });
  }, []);

  // Fetch replay
  useEffect(() => {
    setLoading(true);
    setError(null);
    setCurrentTick(0);
    setPlaying(false);

    const load = USE_MOCKS
      ? Promise.resolve(_buildMockReplay())
      : api.replays.get(id);

    load
      .then((data) => {
        // data may be wrapped in { data: ... } or plain
        setReplay(data?.data ?? data);
      })
      .catch((err) => setError(err.message || 'Failed to load replay'))
      .finally(() => setLoading(false));
  }, [id]);

  // Render loop (RAF)
  const scheduleRender = useCallback(() => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = null;
      const canvas = canvasRef.current;
      const renderFn = renderFnRef.current;
      if (!canvas || !renderFn || !replay) return;
      const ctx = canvas.getContext('2d');
      const state = buildArenaState(replay.frames, currentTickRef.current, replay.config);
      try { renderFn(ctx, state, 'player-1'); } catch { /* ignore render errors */ }
    });
  }, [replay]);

  // Playback tick interval
  useEffect(() => {
    if (!replay) return;
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
    if (!playing) return;

    const totalTicks = replay.frames.length - 1;
    const msPerTick = (1000 / (replay.config?.tick_hz ?? 60)) / speed;

    intervalRef.current = setInterval(() => {
      setCurrentTick((prev) => {
        const next = prev + 1;
        if (next >= totalTicks) {
          setPlaying(false);
          clearInterval(intervalRef.current);
          intervalRef.current = null;
          return totalTicks;
        }
        return next;
      });
    }, msPerTick);

    return () => { clearInterval(intervalRef.current); intervalRef.current = null; };
  }, [playing, speed, replay]);

  // Re-render on tick change
  useEffect(() => { scheduleRender(); }, [currentTick, scheduleRender]);

  // Resize canvas to match container
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const obs = new ResizeObserver(() => {
      const rect = canvas.parentElement.getBoundingClientRect();
      canvas.width = rect.width;
      canvas.height = rect.height;
      scheduleRender();
    });
    obs.observe(canvas.parentElement);
    return () => obs.disconnect();
  }, [scheduleRender]);

  // Cleanup on unmount
  useEffect(() => () => {
    if (rafRef.current) cancelAnimationFrame(rafRef.current);
    if (intervalRef.current) clearInterval(intervalRef.current);
  }, []);

  function handlePlayPause() {
    if (!replay) return;
    const totalTicks = replay.frames.length - 1;
    if (currentTick >= totalTicks) {
      setCurrentTick(0);
    }
    setPlaying((p) => !p);
  }

  function handleSeek(tick) {
    const totalTicks = replay ? replay.frames.length - 1 : 0;
    const clamped = Math.max(0, Math.min(totalTicks, tick));
    setCurrentTick(clamped);
    if (clamped >= totalTicks) setPlaying(false);
  }

  const totalTicks = replay ? replay.frames.length - 1 : 0;
  const verdictClass = replay?.verdict === 'Clean' ? 'replay-verdict--clean' : 'replay-verdict--dirty';

  return (
    <div className="replay-page" role="main" aria-label="Replay viewer">
      {/* Header */}
      <header className="replay-header">
        <div className="replay-header-left">
          <Link to="/spectate/list" className="replay-back-link" aria-label="Back">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M10 3L5 8l5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </Link>
          <h1 className="replay-title">
            <span className="replay-kicker">// replay</span>
            {replay?.id ? <span className="replay-id">{replay.id.slice(0, 8)}</span> : null}
          </h1>
        </div>
        <div className="replay-header-meta">
          {replay && (
            <>
              {replay.recorded_at && (
                <span className="replay-meta-item">
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <rect x="2" y="3" width="12" height="11" rx="2" stroke="currentColor" strokeWidth="1.5" />
                    <path d="M5 1v3M11 1v3M2 7h12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                  </svg>
                  {new Date(replay.recorded_at).toLocaleString()}
                </span>
              )}
              {replay.verdict && (
                <span className={`replay-verdict ${verdictClass}`} aria-label={`Verdict: ${replay.verdict}`}>
                  {replay.verdict === 'Clean' ? (
                    <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="M3 8l4 4 6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                    </svg>
                  ) : (
                    <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                    </svg>
                  )}
                  {replay.verdict}
                </span>
              )}
            </>
          )}
        </div>
      </header>

      {/* Canvas area */}
      <div className="replay-viewport-wrap">
        {loading && (
          <div className="replay-state-overlay" aria-live="polite">
            <div className="replay-spinner" aria-hidden="true" />
            <span>Loading replay…</span>
          </div>
        )}
        {error && (
          <div className="replay-state-overlay replay-state-overlay--error" aria-live="assertive">
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="1.5" />
              <path d="M12 7v5M12 16v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
            <span>{error}</span>
            <button className="replay-retry-btn" onClick={() => window.location.reload()}>
              Retry
            </button>
          </div>
        )}
        {!loading && !error && !replay && (
          <div className="replay-state-overlay" aria-live="polite">
            <span>Replay not found.</span>
          </div>
        )}
        <canvas
          ref={canvasRef}
          className="replay-canvas"
          aria-label="Replay canvas"
          role="img"
        />
      </div>

      {/* Scrubber */}
      {replay && (
        <ReplayScrubber
          currentTick={currentTick}
          totalTicks={totalTicks}
          playing={playing}
          speed={speed}
          onPlay={handlePlayPause}
          onSeek={handleSeek}
          onSpeedChange={setSpeed}
        />
      )}
    </div>
  );
}
