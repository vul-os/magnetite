import { useState, useEffect, useRef, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';

const MOCK_GAMES = [
  { id: 'chess', name: 'Chess', players: 24 },
  { id: 'go', name: 'Go', players: 18 },
  { id: 'checkers', name: 'Checkers', players: 12 },
  { id: 'cards', name: 'Card Master', players: 31 },
];

const MOCK_OPPONENTS = [
  { id: 2, username: 'ChessMaster99', rating: 1850 },
  { id: 3, username: 'GoPlayer42', rating: 2100 },
  { id: 4, username: 'CardShark', rating: 1650 },
];

export default function Matchmaking() {
  const [searchParams] = useSearchParams();
  const [games, setGames] = useState(MOCK_GAMES);
  const [selectedGame, setSelectedGame] = useState(searchParams.get('game') || '');
  const [status, setStatus] = useState('idle');
  const [queueInfo, setQueueInfo] = useState({ waitTime: 0, playersInQueue: 0 });
  const [match, setMatch] = useState(null);
  const wsRef = useRef(null);
  const searchIntervalRef = useRef(null);

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
      if (status === 'searching') {
        setStatus('idle');
      }
    };

    return ws;
  }, [status]);

  useEffect(() => {
    if (status === 'searching') {
      const ws = connectWebSocket(selectedGame);
      searchIntervalRef.current = setInterval(() => {
        setQueueInfo(prev => ({
          waitTime: prev.waitTime + 1,
          playersInQueue: Math.floor(Math.random() * 20) + 5
        }));
      }, 1000);
      return () => {
        ws.close();
        clearInterval(searchIntervalRef.current);
      };
    }
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
    const mockMatch = {
      opponent: MOCK_OPPONENTS[Math.floor(Math.random() * MOCK_OPPONENTS.length)],
      game: games.find(g => g.id === selectedGame),
      sessionId: 'session_' + Date.now(),
      timeControl: '10 min'
    };
    setMatch(mockMatch);
    setStatus('found');
  };

  return (
    <div className="matchmaking-container">
      <h1>Find a Match</h1>

      {status === 'idle' && (
        <div className="matchmaking-card card">
          <div className="form-group">
            <label>Select Game</label>
            <select
              value={selectedGame}
              onChange={(e) => setSelectedGame(e.target.value)}
              className="game-select"
            >
              <option value="">Choose a game...</option>
              {games.map(game => (
                <option key={game.id} value={game.id}>
                  {game.name} ({game.players} online)
                </option>
              ))}
            </select>
          </div>
          <button
            className="btn btn-primary find-match-btn"
            onClick={handleFindMatch}
            disabled={!selectedGame}
          >
            Find Match
          </button>
          <button
            className="btn btn-secondary mock-btn"
            onClick={handleMockMatch}
            disabled={!selectedGame}
          >
            Mock Match (Dev)
          </button>
        </div>
      )}

      {status === 'searching' && (
        <div className="matchmaking-card card searching">
          <div className="spinner"></div>
          <h2>Searching for opponents...</h2>
          <div className="queue-info">
            <div className="queue-stat">
              <span className="queue-label">Wait Time</span>
              <span className="queue-value">{queueInfo.waitTime}s</span>
            </div>
            <div className="queue-stat">
              <span className="queue-label">Players in Queue</span>
              <span className="queue-value">{queueInfo.playersInQueue}</span>
            </div>
          </div>
          <button className="btn btn-secondary cancel-btn" onClick={handleCancel}>
            Cancel
          </button>
        </div>
      )}

      {status === 'found' && match && (
        <div className="matchmaking-card card match-found">
          <div className="match-badge">Match Found!</div>
          <div className="match-details">
            <div className="opponent-info">
              <h3>Your Opponent</h3>
              <div className="opponent-card">
                <span className="opponent-name">{match.opponent.username}</span>
                <span className="opponent-rating">Rating: {match.opponent.rating}</span>
              </div>
            </div>
            <div className="session-info">
              <h3>Game Session</h3>
              <div className="session-card">
                <span>Game: {match.game.name}</span>
                <span>Session: {match.sessionId}</span>
                <span>Time Control: {match.timeControl}</span>
              </div>
            </div>
          </div>
          <button className="btn btn-primary accept-btn">Accept & Play</button>
          <button className="btn btn-secondary decline-btn" onClick={handleCancel}>Decline</button>
        </div>
      )}

      <style>{`
        .matchmaking-container {
          max-width: 600px;
          margin: 2rem auto;
          padding: 0 1rem;
        }
        .matchmaking-container h1 {
          text-align: center;
          margin-bottom: 2rem;
          color: var(--color-text-primary);
        }
        .matchmaking-card {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }
        .form-group {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }
        .form-group label {
          font-weight: 500;
          color: var(--color-text-secondary);
        }
        .game-select {
          padding: 0.75rem;
          background: var(--color-bg-secondary);
          border: 1px solid var(--color-border);
          border-radius: var(--border-radius);
          color: var(--color-text-primary);
          font-size: 1rem;
        }
        .game-select:focus {
          outline: none;
          border-color: var(--color-accent);
        }
        .find-match-btn, .mock-btn {
          width: 100%;
          padding: 0.875rem;
          font-size: 1rem;
        }
        .mock-btn {
          font-size: 0.75rem;
          opacity: 0.5;
        }
        .spinner {
          width: 60px;
          height: 60px;
          margin: 0 auto;
          border: 4px solid var(--color-bg-secondary);
          border-top-color: var(--color-accent);
          border-radius: 50%;
          animation: spin 1s linear infinite;
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        .searching h2 {
          text-align: center;
          color: var(--color-text-primary);
        }
        .queue-info {
          display: flex;
          justify-content: center;
          gap: 3rem;
          padding: 1rem;
          background: var(--color-bg-secondary);
          border-radius: var(--border-radius);
        }
        .queue-stat {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 0.25rem;
        }
        .queue-label {
          font-size: 0.75rem;
          color: var(--color-text-muted);
          text-transform: uppercase;
        }
        .queue-value {
          font-size: 1.25rem;
          font-weight: 600;
          color: var(--color-accent);
        }
        .cancel-btn {
          width: 100%;
        }
        .match-found {
          text-align: center;
        }
        .match-badge {
          display: inline-block;
          padding: 0.5rem 1rem;
          background: var(--color-success);
          color: #000;
          border-radius: var(--border-radius);
          font-weight: 600;
          font-size: 0.875rem;
        }
        .match-details {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }
        .match-details h3 {
          color: var(--color-text-secondary);
          font-size: 0.875rem;
          text-transform: uppercase;
          margin-bottom: 0.5rem;
        }
        .opponent-card, .session-card {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
          padding: 1rem;
          background: var(--color-bg-secondary);
          border-radius: var(--border-radius);
          text-align: left;
        }
        .opponent-name {
          font-size: 1.25rem;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .opponent-rating {
          color: var(--color-text-muted);
        }
        .session-card span {
          color: var(--color-text-secondary);
        }
        .accept-btn, .decline-btn {
          width: 100%;
        }
        .decline-btn {
          margin-top: -0.5rem;
        }
      `}</style>
    </div>
  );
}
