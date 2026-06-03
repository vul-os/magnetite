import { useState, useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { api } from '../api/client';
import { useWebSocket } from '../hooks/useWebSocket';
import './Matchmaking.css';

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────
const MOCK_GAMES = [
  { id: 'void-raiders',  name: 'Void Raiders',  players: 24 },
  { id: 'iron-siege',    name: 'Iron Siege',     players: 18 },
  { id: 'rust-runner',   name: 'Rust Runner',    players: 31 },
  { id: 'orbital-chess', name: 'Orbital Chess',  players: 12 },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Derive the WS base URL from the same env var used by the HTTP client.
// Converts http(s):// → ws(s):// so there is never a hardcoded localhost.
function getWsBase() {
  const apiUrl = import.meta.env.VITE_API_URL || 'http://localhost:8080';
  return apiUrl.replace(/^http(s?):\/\//, (_, s) => `ws${s}://`);
}

export default function Matchmaking() {
  const [searchParams]    = useSearchParams();
  const [games, setGames]               = useState(USE_MOCKS ? MOCK_GAMES : []);
  const [gamesLoading, setGamesLoading] = useState(!USE_MOCKS);
  const [gamesError, setGamesError]     = useState(null);
  const [selectedGame, setSelectedGame] = useState(searchParams.get('game') || '');
  const [status, setStatus]             = useState('idle');
  const [queueInfo, setQueueInfo]       = useState({ waitTime: 0, playersInQueue: 0 });
  const [match, setMatch]               = useState(null);

  // ── Fetch game list ────────────────────────────────────────────────────────
  // Fetch the game list from the API (external system) on mount.
  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setGamesLoading(true);
    setGamesError(null);

    api.games.list()
      .then((data) => {
        if (!cancelled) {
          const list = Array.isArray(data?.games) ? data.games : (Array.isArray(data) ? data : []);
          setGames(list);
        }
      })
      .catch((err) => {
        if (!cancelled) setGamesError(err.message ?? 'Failed to load games');
      })
      .finally(() => {
        if (!cancelled) setGamesLoading(false);
      });

    return () => { cancelled = true; };
  }, []);

  // ── Matchmaking status polling (while searching) ───────────────────────────
  const statusIntervalRef = useRef(null);

  // ── WebSocket for real-time matchmaking events ─────────────────────────────
  // Only activate the WS hook when we have a selected game and are searching.
  const wsPath = status === 'searching' && selectedGame
    ? `${getWsBase()}/ws/matchmaking/${selectedGame}`
    : null;

  // We use the hook conditionally based on wsPath being set.
  // Since hooks cannot be called conditionally, we pass an empty-ish URL
  // when not searching and suppress the connection via the disabled pattern.
  const {
    isConnected: wsConnected,
    lastMessage: wsMessage,
    sendMessage: wsSend,
  } = useWebSocket(wsPath ?? '/ws/matchmaking/_noop', {
    autoReconnect: wsPath !== null,
  });

  // Send join_queue once connected
  useEffect(() => {
    if (wsConnected && status === 'searching') {
      wsSend({ type: 'join_queue' });
    }
  }, [wsConnected, status, wsSend]);

  // Handle WS messages from the matchmaking server (external system).
  useEffect(() => {
    if (!wsMessage) return;
    if (wsMessage.type === 'queue_update') {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setQueueInfo({
        waitTime: wsMessage.waitTime ?? wsMessage.wait_seconds ?? queueInfo.waitTime,
        playersInQueue: wsMessage.playersInQueue ?? wsMessage.players_in_queue ?? queueInfo.playersInQueue,
      });
    } else if (wsMessage.type === 'match_found') {
      setMatch(wsMessage.match);
      setStatus('found');
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wsMessage]);

  // Poll matchmaking status via REST as a complement to the WS
  useEffect(() => {
    if (status !== 'searching') return;

    // Increment wait time locally; real queue count comes from WS/status API
    statusIntervalRef.current = setInterval(async () => {
      setQueueInfo(prev => ({ ...prev, waitTime: prev.waitTime + 1 }));

      // Also poll the REST status endpoint to sync queue count
      try {
        const data = await api.matchmaking.status();
        if (data?.players_in_queue != null) {
          setQueueInfo(prev => ({ ...prev, playersInQueue: data.players_in_queue }));
        }
        if (data?.status === 'matched' && data?.match) {
          setMatch(data.match);
          setStatus('found');
        }
      } catch {
        // Ignore polling errors — WS is the primary channel
      }
    }, 1000);

    return () => clearInterval(statusIntervalRef.current);
  }, [status]);

  const handleFindMatch = () => {
    if (!selectedGame) return;
    setStatus('searching');
    setMatch(null);
    setQueueInfo({ waitTime: 0, playersInQueue: 0 });
    // Join via REST as well (backend may require an explicit join call)
    api.matchmaking.join(selectedGame).catch(() => {/* WS is primary, ignore */});
  };

  const handleCancel = () => {
    clearInterval(statusIntervalRef.current);
    wsSend({ type: 'leave_queue' });
    api.matchmaking.leave().catch(() => {});
    setStatus('idle');
    setQueueInfo({ waitTime: 0, playersInQueue: 0 });
  };

  return (
    <div className="matchmaking-page" role="main">
      <span className="matchmaking-kicker">// Find a Rust Match</span>
      <h1>Matchmaking</h1>

      {/* ── Idle ── */}
      {status === 'idle' && (
        <div className="matchmaking-card" role="region" aria-label="Game selection">
          <div>
            <label className="game-select-label" htmlFor="mm-game-select">
              // Select Game
            </label>

            {gamesLoading && (
              <p className="matchmaking-loading" aria-live="polite">Loading games…</p>
            )}

            {gamesError && !gamesLoading && (
              <p className="matchmaking-error" role="alert">{gamesError}</p>
            )}

            {!gamesLoading && (
              <select
                id="mm-game-select"
                value={selectedGame}
                onChange={(e) => setSelectedGame(e.target.value)}
                className="game-select"
                disabled={gamesLoading}
              >
                <option value="">Choose a Rust game…</option>
                {games.map(g => (
                  <option key={g.id} value={g.id}>
                    {g.name}{g.players != null ? ` — ${g.players} online` : ''}
                  </option>
                ))}
              </select>
            )}
          </div>

          <button
            className="find-match-btn"
            onClick={handleFindMatch}
            disabled={!selectedGame || gamesLoading}
          >
            Find Match
          </button>
        </div>
      )}

      {/* ── Searching ── */}
      {status === 'searching' && (
        <div className="matchmaking-card searching-state" role="status" aria-live="polite" aria-label="Searching for opponents">
          <div className="search-spinner-wrap" aria-hidden="true">
            <div className="search-ring" />
            <div className="search-ring-inner" />
          </div>

          <h2>Searching for opponents…</h2>

          <div className="queue-stats" aria-label="Queue information">
            <div className="queue-stat">
              <span className="queue-stat-label">Wait Time</span>
              <span className="queue-stat-value" aria-live="off">{queueInfo.waitTime}s</span>
            </div>
            <div className="queue-stat">
              <span className="queue-stat-label">In Queue</span>
              <span className="queue-stat-value" aria-live="off">
                {queueInfo.playersInQueue > 0 ? queueInfo.playersInQueue : '—'}
              </span>
            </div>
          </div>

          <button className="cancel-btn" onClick={handleCancel}>
            Cancel Search
          </button>
        </div>
      )}

      {/* ── Match found ── */}
      {status === 'found' && match && (
        <div className="matchmaking-card match-found-state" role="alert" aria-label="Match found">
          <div className="match-found-badge">
            <div className="match-found-dot" aria-hidden="true" />
            Match Found
          </div>

          <div className="match-details">
            <div>
              <span className="match-section-label">// Your Opponent</span>
              <div className="opponent-card">
                <span className="opponent-name">{match.opponent?.username ?? match.opponent_username}</span>
                <span className="opponent-rating">Rating: {match.opponent?.rating ?? match.opponent_rating ?? '—'}</span>
              </div>
            </div>

            <div>
              <span className="match-section-label">// Session Details</span>
              <div className="session-card">
                <div className="session-row">
                  <span className="session-key">Game</span>
                  <span className="session-val">{match.game?.name ?? match.game_name ?? selectedGame}</span>
                </div>
                <div className="session-row">
                  <span className="session-key">Session ID</span>
                  <span className="session-val">{match.sessionId ?? match.session_id}</span>
                </div>
                {match.timeControl || match.time_control ? (
                  <div className="session-row">
                    <span className="session-key">Time Control</span>
                    <span className="session-val">{match.timeControl ?? match.time_control}</span>
                  </div>
                ) : null}
              </div>
            </div>
          </div>

          <button className="accept-btn">
            ▶  Accept &amp; Play
          </button>
          <button className="decline-btn" onClick={handleCancel}>
            Decline
          </button>
        </div>
      )}
    </div>
  );
}
