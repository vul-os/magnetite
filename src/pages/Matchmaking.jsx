import { useState, useEffect, useRef, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import './Matchmaking.css';

const MOCK_GAMES = [
  { id: 'void-raiders',  name: 'Void Raiders',  players: 24 },
  { id: 'iron-siege',    name: 'Iron Siege',     players: 18 },
  { id: 'rust-runner',   name: 'Rust Runner',    players: 31 },
  { id: 'orbital-chess', name: 'Orbital Chess',  players: 12 },
];

const MOCK_OPPONENTS = [
  { id: 2, username: 'ChessMaster99', rating: 1850 },
  { id: 3, username: 'GoPlayer42',    rating: 2100 },
  { id: 4, username: 'CardShark',     rating: 1650 },
];

export default function Matchmaking() {
  const [searchParams]    = useSearchParams();
  const games              = MOCK_GAMES;
  const [selectedGame, setSelectedGame] = useState(searchParams.get('game') || '');
  const [status, setStatus]             = useState('idle');
  const [queueInfo, setQueueInfo]       = useState({ waitTime: 0, playersInQueue: 0 });
  const [match, setMatch]               = useState(null);

  const wsRef              = useRef(null);
  const searchIntervalRef  = useRef(null);

  const connectWebSocket = useCallback((gameId) => {
    const ws = new WebSocket(`ws://localhost:3000/ws/game/${gameId}`);
    wsRef.current = ws;

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: 'join_queue' }));
    };

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.type === 'queue_update') {
        setQueueInfo({ waitTime: data.waitTime, playersInQueue: data.playersInQueue });
      } else if (data.type === 'match_found') {
        setMatch(data.match);
        setStatus('found');
      }
    };

    ws.onclose = () => {
      setStatus(prev => prev === 'searching' ? 'idle' : prev);
    };

    return ws;
  }, []);

  useEffect(() => {
    if (status !== 'searching') return;
    const ws = connectWebSocket(selectedGame);
    searchIntervalRef.current = setInterval(() => {
      setQueueInfo(prev => ({
        waitTime: prev.waitTime + 1,
        playersInQueue: Math.floor(Math.random() * 20) + 5,
      }));
    }, 1000);

    return () => {
      ws.close();
      clearInterval(searchIntervalRef.current);
    };
  }, [status, selectedGame, connectWebSocket]);

  const handleFindMatch = () => {
    if (!selectedGame) return;
    setStatus('searching');
    setMatch(null);
    setQueueInfo({ waitTime: 0, playersInQueue: 0 });
  };

  const handleCancel = () => {
    if (wsRef.current) {
      wsRef.current.send(JSON.stringify({ type: 'leave_queue' }));
      wsRef.current.close();
    }
    clearInterval(searchIntervalRef.current);
    setStatus('idle');
    setQueueInfo({ waitTime: 0, playersInQueue: 0 });
  };

  const handleMockMatch = () => {
    const opponent = MOCK_OPPONENTS[Math.floor(Math.random() * MOCK_OPPONENTS.length)];
    const game     = games.find(g => g.id === selectedGame);
    setMatch({ opponent, game, sessionId: `sess_${Date.now()}`, timeControl: '10 min' });
    setStatus('found');
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
            <select
              id="mm-game-select"
              value={selectedGame}
              onChange={(e) => setSelectedGame(e.target.value)}
              className="game-select"
            >
              <option value="">Choose a Rust game…</option>
              {games.map(g => (
                <option key={g.id} value={g.id}>
                  {g.name} — {g.players} online
                </option>
              ))}
            </select>
          </div>

          <button
            className="find-match-btn"
            onClick={handleFindMatch}
            disabled={!selectedGame}
          >
            Find Match
          </button>

          <button
            className="cancel-btn"
            onClick={handleMockMatch}
            disabled={!selectedGame}
            style={{ opacity: 0.5, fontSize: 'var(--text-xs)' }}
          >
            Simulate Match Found (Dev)
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
              <span className="queue-stat-value" aria-live="off">{queueInfo.playersInQueue}</span>
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
                <span className="opponent-name">{match.opponent.username}</span>
                <span className="opponent-rating">Rating: {match.opponent.rating}</span>
              </div>
            </div>

            <div>
              <span className="match-section-label">// Session Details</span>
              <div className="session-card">
                <div className="session-row">
                  <span className="session-key">Game</span>
                  <span className="session-val">{match.game?.name}</span>
                </div>
                <div className="session-row">
                  <span className="session-key">Session ID</span>
                  <span className="session-val">{match.sessionId}</span>
                </div>
                <div className="session-row">
                  <span className="session-key">Time Control</span>
                  <span className="session-val">{match.timeControl}</span>
                </div>
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
