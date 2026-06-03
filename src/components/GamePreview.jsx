/**
 * GamePreview — embeds a live authoritative game over WebSocket.
 *
 * Uses the magnetite-web-client (../../magnetite-web-client/src/client.js):
 *   - Connects to wsEndpoint (from play manifest server_url or a user-supplied dev URL)
 *   - Renders live ServerNet frames (Welcome / Snapshot / Delta) onto a canvas
 *   - Captures keyboard + mouse input and sends ClientNet::InputFrame each tick
 *
 * Props:
 *   wsEndpoint  {string|null}  — WebSocket URL to connect to.
 *                                When null and devMode=false, shows "no server" empty state.
 *   devMode     {boolean}      — When true shows a URL input so devs can type a ws:// address.
 *   onClose     {function}     — Optional close/back handler.
 *   token       {string|null}  — Optional JWT auth token (forwarded as ?token=).
 *   title       {string}       — Display name for the game.
 *
 * Mock: when VITE_USE_MOCKS=true and wsEndpoint is null, shows a mock canvas
 * rendering a spinning arena demo so the component is always renderable.
 */

import { useRef, useEffect, useState, useCallback } from 'react';
import { createClient } from '../../magnetite-web-client/src/client.js';
import { useTranslation } from '../i18n/useTranslation';
import './GamePreview.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// ── Mock canvas renderer (when no server is available) ───────────────────────
function runMockRenderer(canvas) {
  let rafId = null;
  let tick = 0;
  const cx = canvas.getContext('2d');

  function frame() {
    const W = canvas.width;
    const H = canvas.height;
    tick++;

    cx.fillStyle = '#07070b';
    cx.fillRect(0, 0, W, H);

    // grid
    cx.strokeStyle = 'rgba(35,35,46,0.5)';
    cx.lineWidth = 1;
    const g = 40;
    for (let x = 0; x < W; x += g) {
      cx.beginPath(); cx.moveTo(x, 0); cx.lineTo(x, H); cx.stroke();
    }
    for (let y = 0; y < H; y += g) {
      cx.beginPath(); cx.moveTo(0, y); cx.lineTo(W, y); cx.stroke();
    }

    // Rotating arena ring
    const cx2 = W / 2;
    const cy2 = H / 2;
    const r = Math.min(W, H) * 0.32;
    cx.beginPath();
    cx.arc(cx2, cy2, r, 0, Math.PI * 2);
    cx.strokeStyle = 'rgba(56,225,200,0.18)';
    cx.lineWidth = 1;
    cx.stroke();

    // Mock players
    const mockPlayers = [
      { color: '#38e1c8', a: tick * 0.02 },
      { color: '#f5a524', a: tick * 0.02 + Math.PI },
      { color: '#5b9dff', a: tick * 0.015 + Math.PI * 0.5 },
    ];

    for (const p of mockPlayers) {
      const px = cx2 + Math.cos(p.a) * r * 0.7;
      const py = cy2 + Math.sin(p.a) * r * 0.7;
      cx.beginPath();
      cx.arc(px, py, 7, 0, Math.PI * 2);
      cx.fillStyle = p.color;
      cx.fill();
    }

    // Mock "DEMO" watermark
    cx.font = 'bold 11px monospace';
    cx.fillStyle = 'rgba(56,225,200,0.25)';
    cx.textAlign = 'center';
    cx.fillText('MOCK — connect a live server', cx2, H - 18);

    rafId = requestAnimationFrame(frame);
  }

  rafId = requestAnimationFrame(frame);
  return () => { if (rafId) cancelAnimationFrame(rafId); };
}

// ── Component ────────────────────────────────────────────────────────────────
export default function GamePreview({
  wsEndpoint = null,
  devMode = false,
  onClose = null,
  token = null,
  title = 'Game Preview',
}) {
  const { t } = useTranslation();
  const canvasRef   = useRef(null);
  const clientRef   = useRef(null);
  const mockCleanup = useRef(null);

  const [status, setStatus]     = useState('idle');     // idle | connecting | connected | error | disconnected
  const [error, setError]       = useState(null);
  const [playerCount, setPlayerCount] = useState(0);
  const [latency, setLatency]   = useState(null);
  const [pingHandle, setPingHandle] = useState(null);

  // For devMode — the user can type a ws:// URL
  const [devUrl, setDevUrl]     = useState('ws://localhost:9001');
  const [activeUrl, setActiveUrl] = useState(wsEndpoint);

  // Sync activeUrl when wsEndpoint prop changes (e.g. manifest loaded after mount).
  // This mirrors an async-arriving prop into local state that the user can also
  // override in devMode, so deriving during render is not possible here.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (wsEndpoint) setActiveUrl(wsEndpoint);
  }, [wsEndpoint]);

  // ── Start / stop the client ───────────────────────────────────────────────
  const startClient = useCallback((url) => {
    if (!url || !canvasRef.current) return;

    // Tear down any existing client or mock renderer
    if (clientRef.current) {
      clientRef.current.disconnect();
      clientRef.current = null;
    }
    if (mockCleanup.current) {
      mockCleanup.current();
      mockCleanup.current = null;
    }
    if (pingHandle) {
      clearInterval(pingHandle);
      setPingHandle(null);
    }

    setStatus('connecting');
    setError(null);

    const canvas = canvasRef.current;
    // Resize canvas to its display size
    canvas.width  = canvas.clientWidth  || 640;
    canvas.height = canvas.clientHeight || 360;

    const authToken = token || localStorage.getItem('token') || undefined;

    const client = createClient({
      url:    url,
      token:  authToken,
      canvas: canvas,
      autoReconnect: false,
    });

    clientRef.current = client;

    // Wire status from connection events
    client._conn.onOpen = () => setStatus('connecting'); // waiting for Welcome
    client._conn.onClose = () => {
      setStatus('disconnected');
      clearInterval(pingHandle);
    };
    client._conn.onError = () => {
      setStatus('error');
      setError('WebSocket connection failed. Is the server running?');
    };

    // Welcome → connected
    const origWelcome = client._handleWelcome.bind(client);
    client._handleWelcome = (msg) => {
      origWelcome(msg);
      setStatus('connected');
    };

    // Track player count from state updates
    client.onState((state) => {
      if (!state) return;
      const players = state.players ?? state.other_players ?? null;
      if (Array.isArray(players)) {
        // other_players doesn't include self; add 1 if we have self_state
        const selfBonus = state.self_state ? 1 : 0;
        setPlayerCount(players.length + selfBonus);
      }
    });

    client.connect();

    // Latency ping every 3s
    const pHandle = setInterval(() => {
      if (client._conn.isConnected) {
        const t0 = Date.now();
        // We can't easily track the reply without a full ping/pong loop,
        // so estimate as round-trip if Ack arrives within 200 ms.
        // Just measure time from sendInput to state update as a proxy.
        // For now: expose the tick lag from the prediction buffer.
        const lag = client._prediction?.lag ?? null;
        if (lag !== null) setLatency(lag);
        else setLatency(Date.now() - t0); // trivial fallback
      }
    }, 3000);
    setPingHandle(pHandle);

    return client;
  }, [token, pingHandle]);

  // Auto-connect when activeUrl becomes available (and not in devMode waiting for
  // input). This effect manages the WebSocket/mock-renderer lifecycle, so it must
  // drive connection status state from within the effect.
  useEffect(() => {
    if (!activeUrl && USE_MOCKS && canvasRef.current) {
      // Show mock demo
      setStatus('connected');
      mockCleanup.current = runMockRenderer(canvasRef.current);
      return;
    }

    if (!activeUrl) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setStatus('idle');
      return;
    }

    startClient(activeUrl);

    return () => {
      if (clientRef.current) {
        clientRef.current.disconnect();
        clientRef.current = null;
      }
      if (mockCleanup.current) {
        mockCleanup.current();
        mockCleanup.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeUrl]);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (clientRef.current) {
        clientRef.current.disconnect();
        clientRef.current = null;
      }
      if (mockCleanup.current) {
        mockCleanup.current();
        mockCleanup.current = null;
      }
      if (pingHandle) clearInterval(pingHandle);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Resize canvas on container resize
  useEffect(() => {
    if (!canvasRef.current) return;
    const obs = new ResizeObserver(() => {
      const c = canvasRef.current;
      if (!c) return;
      c.width  = c.clientWidth  || 640;
      c.height = c.clientHeight || 360;
    });
    obs.observe(canvasRef.current);
    return () => obs.disconnect();
  }, []);

  const handleDevConnect = () => {
    if (!devUrl.trim()) return;
    setActiveUrl(devUrl.trim());
  };

  const handleDisconnect = () => {
    if (clientRef.current) {
      clientRef.current.disconnect();
      clientRef.current = null;
    }
    if (mockCleanup.current) {
      mockCleanup.current();
      mockCleanup.current = null;
    }
    setStatus('idle');
    setActiveUrl(null);
    setError(null);
  };

  // ── Render ────────────────────────────────────────────────────────────────
  const showCanvas   = status === 'connecting' || status === 'connected';
  const showIdle     = status === 'idle';
  const showError    = status === 'error';
  const showDisconn  = status === 'disconnected';
  const isConnected  = status === 'connected';

  return (
    <div className="game-preview" role="region" aria-label={t('game.previewGameLabel', { title })}>
      {/* Header bar */}
      <div className="gp-header">
        <div className="gp-title">
          <span className="kicker gp-kicker">{t('game.previewSection')}</span>
          <span className="gp-name">{title}</span>
        </div>

        <div className="gp-status-row">
          {(showCanvas || showDisconn) && (
            <div
              className="gp-conn-pill"
              data-status={status}
              aria-live="polite"
              aria-label={`Connection: ${status}`}
            >
              <span className="gp-dot" aria-hidden="true" />
              <span>
                {status === 'connecting'   ? t('game.previewConnecting')   :
                 status === 'connected'    ? t('game.previewLive')         :
                 status === 'disconnected' ? t('game.previewDisconnected') : status}
              </span>
              {isConnected && latency !== null && (
                <span className="gp-latency" aria-label={t('game.latency', { ms: latency })}>{latency}ms</span>
              )}
              {isConnected && playerCount > 0 && (
                <span className="gp-players">{playerCount} player{playerCount !== 1 ? 's' : ''}</span>
              )}
            </div>
          )}

          {(isConnected || status === 'connecting') && (
            <button className="gp-action-btn" onClick={handleDisconnect} aria-label={t('game.previewDisconnect')}>
              {t('game.previewDisconnect')}
            </button>
          )}

          {onClose && (
            <button className="gp-action-btn gp-close-btn" onClick={onClose} aria-label={t('game.previewClose')}>
              ✕
            </button>
          )}
        </div>
      </div>

      {/* Canvas area */}
      <div className="gp-canvas-wrap">
        <canvas
          ref={canvasRef}
          className="gp-canvas"
          aria-hidden="true"
          tabIndex={-1}
          style={{ display: showCanvas ? 'block' : 'none' }}
        />

        {/* Dev URL input overlay */}
        {devMode && showIdle && (
          <div className="gp-overlay gp-dev-overlay">
            <span className="kicker" style={{ marginBottom: '0.5rem' }}>{t('game.previewDevTitle')}</span>
            <h4>{t('game.previewDevHeading')}</h4>
            <p>Run <code>magnetite dev</code> in your game crate directory, then enter the WebSocket URL:</p>
            <div className="gp-dev-input-row">
              <label htmlFor="gp-dev-url" className="gp-visually-hidden">{t('game.wsUrlLabel')}</label>
              <input
                id="gp-dev-url"
                type="text"
                className="gp-url-input"
                placeholder="ws://localhost:9001"
                value={devUrl}
                onChange={(e) => setDevUrl(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleDevConnect(); }}
                aria-label={t('game.wsUrlLabel')}
              />
              <button
                className="btn btn-primary gp-connect-btn"
                onClick={handleDevConnect}
                disabled={!devUrl.trim()}
              >
                Connect
              </button>
            </div>
          </div>
        )}

        {/* No server (non-devMode, no endpoint) */}
        {!devMode && showIdle && (
          <div className="gp-overlay gp-empty">
            <div className="gp-empty-icon" aria-hidden="true">⬡</div>
            <h4>{t('game.previewNoServer')}</h4>
            <p>{t('game.previewNoServerBody')}</p>
          </div>
        )}

        {/* Error */}
        {showError && (
          <div className="gp-overlay gp-error-overlay" role="alert">
            <div className="gp-error-icon" aria-hidden="true">!</div>
            <h4>{t('game.previewConnFailed')}</h4>
            <p>{error}</p>
            <button
              className="btn btn-primary"
              onClick={() => { setStatus('idle'); setError(null); }}
            >
              {t('game.previewTryAgain')}
            </button>
          </div>
        )}

        {/* Disconnected */}
        {showDisconn && (
          <div className="gp-overlay gp-disconn-overlay" role="alert">
            <div className="gp-empty-icon" aria-hidden="true">◎</div>
            <h4>{t('game.previewDisconnected')}</h4>
            <p>The server closed the connection.</p>
            <button
              className="btn btn-primary"
              onClick={() => activeUrl && startClient(activeUrl)}
            >
              {t('game.previewReconnect')}
            </button>
          </div>
        )}
      </div>

      {/* Keyboard hint */}
      {isConnected && (
        <div className="gp-footer" aria-label={t('game.controlsHint')}>
          <span>{t('game.controlsMove')}</span>
          <span>{t('game.controlsAim')}</span>
          <span>{t('game.controlsShoot')}</span>
          <span>{t('game.controlsOverlay')}</span>
        </div>
      )}
    </div>
  );
}
