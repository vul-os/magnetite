import { useState, useEffect, useCallback } from 'react';
import { useNavigate, useParams, Link } from 'react-router-dom';
import { useWebSocket } from '../hooks/useWebSocket';
import GameOverlay from '../components/GameOverlay';
import { api } from '../api/client';
import './Spectator.css';

// ── Mock replays ──────────────────────────────────────────────────────────────
const MOCK_REPLAYS = [
  { id: 'mock-replay-1', recorded_at: new Date(Date.now() - 3600000).toISOString(), tick_count: 300, verdict: 'Clean' },
  { id: 'mock-replay-2', recorded_at: new Date(Date.now() - 86400000).toISOString(), tick_count: 540, verdict: 'Clean' },
];

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────
const MOCK_SPECTATORS = [
  { id: 1, username: 'Viewer123',   isChatting: true  },
  { id: 2, username: 'GamerFan',    isChatting: false },
  { id: 3, username: 'Spectator99', isChatting: true  },
];

const MOCK_PLAYERS = [
  { id: 1, username: 'PlayerOne',  score: 1250, kills: 5, deaths: 2, position: { x: 30, y: 45 } },
  { id: 2, username: 'GameMaster', score: 1100, kills: 4, deaths: 3, position: { x: 55, y: 20 } },
  { id: 3, username: 'ProGamer99', score:  980, kills: 3, deaths: 4, position: { x: 70, y: 60 } },
  { id: 4, username: 'NoobMaster', score:  720, kills: 2, deaths: 5, position: { x: 20, y: 75 } },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export default function Spectator() {
  const { id: gameId }  = useParams();
  const navigate        = useNavigate();

  const { isConnected, lastMessage, sendMessage } = useWebSocket(
    `/ws/spectate/${gameId}`
  );

  const [players, setPlayers]           = useState(USE_MOCKS ? MOCK_PLAYERS : []);
  const [cameraMode, setCameraMode]     = useState('follow');
  const [followedPlayer, setFollowedPlayer] = useState(USE_MOCKS ? MOCK_PLAYERS[0] : null);
  const [spectators, setSpectators]     = useState(USE_MOCKS ? MOCK_SPECTATORS : []);
  const [chatMessages, setChatMessages] = useState(
    USE_MOCKS
      ? [
          { id: 1, player: 'Viewer123', message: 'Great game!',      timestamp: 60000 },
          { id: 2, player: 'GamerFan',  message: 'This is intense!', timestamp: 30000 },
        ]
      : []
  );
  const [chatInput, setChatInput]       = useState('');
  const [replays, setReplays]           = useState([]);
  const [replaysLoading, setReplaysLoading] = useState(false);

  // camera position derived from followedPlayer — no extra state needed
  const cameraTranslate = cameraMode === 'follow' && followedPlayer
    ? `translate(${-followedPlayer.position.x}%, ${-followedPlayer.position.y}%)`
    : 'translate(0, 0)';

  // Send join_spectate once the WS connects
  useEffect(() => {
    if (isConnected) {
      sendMessage({ type: 'join_spectate', gameId });
    }
  }, [isConnected, gameId, sendMessage]);

  // Handle incoming WS messages
  useEffect(() => {
    if (!lastMessage) return;

    switch (lastMessage.type) {
      case 'players_update':
        // eslint-disable-next-line react-hooks/set-state-in-effect
        setPlayers(lastMessage.players);
        if (cameraMode === 'follow') {
          const updated = lastMessage.players.find(p => p.id === followedPlayer?.id);
          if (updated) setFollowedPlayer(updated);
        }
        break;
      case 'spectator_update':
        setSpectators(lastMessage.spectators);
        break;
      case 'chat_message':
        setChatMessages(prev => [...prev, lastMessage.message]);
        break;
      case 'game_ended':
        navigate('/matchmaking');
        break;
      default:
        break;
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lastMessage]);

  // Fetch recent replays for this game
  // Fetch recent replays from the API (external system).
  useEffect(() => {
    if (!gameId) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setReplaysLoading(true);
    // GET /api/v1/replays does not exist; replays are listed per game at
    // GET /api/v1/games/:id/replays, which is what this page actually wants.
    const fetch = USE_MOCKS
      ? Promise.resolve(MOCK_REPLAYS)
      : api.replays.listForGame(gameId, { limit: 5 }).then((r) => r?.data ?? r);
    fetch
      .then((r) => setReplays(Array.isArray(r) ? r : []))
      .catch(() => setReplays([]))
      .finally(() => setReplaysLoading(false));
  }, [gameId]);

  const handleSendChat = useCallback((e) => {
    e.preventDefault();
    if (!chatInput.trim()) return;

    setChatMessages(prev => [
      ...prev,
      { id: Date.now(), player: 'You', message: chatInput, timestamp: Date.now() },
    ]);

    sendMessage({ type: 'spectator_chat', message: chatInput });
    setChatInput('');
  }, [chatInput, sendMessage]);

  const handleExit = useCallback(() => {
    sendMessage({ type: 'leave_spectate' });
    navigate('/matchmaking');
  }, [sendMessage, navigate]);

  const handleFollowPlayer = useCallback((playerId) => {
    const p = players.find(pl => pl.id === parseInt(playerId, 10));
    if (p) { setFollowedPlayer(p); setCameraMode('follow'); }
  }, [players]);

  // sort players by score descending for leaderboard
  const sortedPlayers = [...players].sort((a, b) => b.score - a.score);

  // Loading state when no data yet
  const isLoading = !USE_MOCKS && !isConnected && players.length === 0;

  return (
    <div className="spectator-container" role="main" aria-label="Spectator view">
      {/* Game map */}
      <div className="spectator-viewport">
        {isLoading ? (
          <div className="spectator-connecting" aria-live="polite">
            <span className="kicker">// connecting…</span>
          </div>
        ) : (
          <div className="game-world" style={{ transform: cameraTranslate }}>
            <div className="game-map" aria-hidden="true">
              {players.map(player => (
                <div
                  key={player.id}
                  className={`player-marker ${cameraMode === 'follow' && followedPlayer?.id === player.id ? 'followed' : ''}`}
                  style={{ left: `${player.position.x}%`, top: `${player.position.y}%` }}
                  role="button"
                  tabIndex={0}
                  aria-label={`Follow ${player.username}`}
                  onClick={() => handleFollowPlayer(player.id)}
                  onKeyDown={e => e.key === 'Enter' && handleFollowPlayer(player.id)}
                >
                  <div className="player-icon" />
                  <span className="player-label">{player.username}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* HUD overlay */}
      <div className="spectator-overlay" aria-label="Spectator HUD">
        {/* Top bar */}
        <header className="spectator-header">
          <div className="camera-mode-selector" role="group" aria-label="Camera mode">
            <button
              className={`mode-btn ${cameraMode === 'follow' ? 'active' : ''}`}
              onClick={() => setCameraMode('follow')}
              aria-pressed={cameraMode === 'follow'}
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="2" />
                <path d="M8 1v2M8 13v2M1 8h2M13 8h2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              Follow
            </button>
            <button
              className={`mode-btn ${cameraMode === 'free' ? 'active' : ''}`}
              onClick={() => setCameraMode('free')}
              aria-pressed={cameraMode === 'free'}
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <rect x="2" y="2" width="12" height="12" rx="2" stroke="currentColor" strokeWidth="2" />
                <path d="M8 5v6M5 8h6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              Free Camera
            </button>
          </div>

          <div className="spectators-count" aria-live="polite" aria-label={`${spectators.length} spectators watching`}>
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <circle cx="8" cy="6" r="3" stroke="currentColor" strokeWidth="2" />
              <path d="M2 14c0-3 2.5-5 6-5s6 2 6 5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
            <span>{spectators.length} watching</span>
          </div>

          <button className="spectator-exit-btn" onClick={handleExit}>
            Exit Spectator
          </button>
        </header>

        {cameraMode === 'follow' && (
          <div className="followed-player-bar">
            <span className="followed-label">// Following</span>
            <select
              value={followedPlayer?.id || ''}
              onChange={(e) => handleFollowPlayer(e.target.value)}
              className="player-select"
              aria-label="Select player to follow"
            >
              {players.map(p => (
                <option key={p.id} value={p.id}>
                  {p.username} — {p.score.toLocaleString()} pts
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Leaderboard panel */}
        <aside className="spectator-leaderboard" aria-label="Player leaderboard">
          <h3 className="spectator-panel-title">// Players</h3>
          {players.length === 0 && !isLoading && (
            <p className="spectator-empty">No player data yet.</p>
          )}
          <div className="spectator-players-list" role="list">
            {sortedPlayers.map((player, idx) => (
              <div
                key={player.id}
                role="listitem"
                className={`spectator-player-row ${cameraMode === 'follow' && followedPlayer?.id === player.id ? 'followed' : ''}`}
                onClick={() => handleFollowPlayer(player.id)}
                style={{ cursor: 'pointer' }}
                tabIndex={0}
                onKeyDown={e => e.key === 'Enter' && handleFollowPlayer(player.id)}
                aria-label={`${player.username}, rank ${idx + 1}, score ${player.score}`}
              >
                <span className="spectator-rank">#{idx + 1}</span>
                <div className="spectator-player-info">
                  <span className="spectator-player-name">{player.username}</span>
                  <span className="spectator-player-stats">{player.kills}K / {player.deaths}D</span>
                </div>
                <span className="spectator-player-score">{player.score.toLocaleString()}</span>
              </div>
            ))}
          </div>
        </aside>

        {/* Spectator chat */}
        <aside className="spectator-chat" aria-label="Spectator chat">
          <h3 className="spectator-panel-title">// Spectator Chat</h3>
          <div className="spectator-chat-messages" aria-live="polite">
            {chatMessages.length === 0 && (
              <p className="spectator-empty">No messages yet.</p>
            )}
            {chatMessages.map(msg => (
              <div key={msg.id} className="spectator-chat-msg">
                <span className="spectator-chat-player">{msg.player}:</span>
                <span className="spectator-chat-text">{msg.message}</span>
              </div>
            ))}
          </div>
          <form className="spectator-chat-form" onSubmit={handleSendChat}>
            <input
              type="text"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              placeholder="Send a message…"
              className="spectator-chat-input"
              aria-label="Chat message"
            />
            <button type="submit" className="spectator-send-btn" aria-label="Send">
              Send
            </button>
          </form>
        </aside>
      </div>

      {/* Replays panel ─ links to /replay/:id */}
      <aside className="spectator-replays" aria-label="Recent replays">
        <h3 className="spectator-panel-title">// replays</h3>
        {replaysLoading && (
          <p className="spectator-empty">Loading…</p>
        )}
        {!replaysLoading && replays.length === 0 && (
          <p className="spectator-empty">No replays yet.</p>
        )}
        <ul className="spectator-replays-list" role="list">
          {replays.map((r) => (
            <li key={r.id} role="listitem">
              <Link
                to={`/replay/${r.id}`}
                className="spectator-replay-link"
                aria-label={`Watch replay from ${new Date(r.recorded_at).toLocaleString()}`}
              >
                <span className="spectator-replay-id">{r.id.slice(0, 8)}</span>
                <span className="spectator-replay-meta">
                  {new Date(r.recorded_at).toLocaleDateString()}
                  {r.tick_count ? ` · ${r.tick_count} ticks` : ''}
                </span>
                <span className={`spectator-replay-verdict ${r.verdict === 'Clean' ? 'srv--clean' : 'srv--dirty'}`}>
                  {r.verdict || '—'}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      </aside>

      {/* In-game comms overlay — chat + voice (Tab / ` to toggle) */}
      <GameOverlay
        label="Spectator"
        channelId={gameId ? `spectate-${gameId}` : null}
        voiceRoomId={null}
      />
    </div>
  );
}
