import { useState, useEffect, useRef } from 'react';
import { useNavigate, useParams } from 'react-router-dom';

const MOCK_SPECTATORS = [
  { id: 1, username: 'Viewer123', isChatting: true },
  { id: 2, username: 'GamerFan', isChatting: false },
  { id: 3, username: 'Spectator99', isChatting: true },
];

const MOCK_PLAYERS = [
  { id: 1, username: 'PlayerOne', score: 1250, kills: 5, deaths: 2, position: { x: 30, y: 45 } },
  { id: 2, username: 'GameMaster', score: 1100, kills: 4, deaths: 3, position: { x: 55, y: 20 } },
  { id: 3, username: 'ProGamer99', score: 980, kills: 3, deaths: 4, position: { x: 70, y: 60 } },
  { id: 4, username: 'NoobMaster', score: 720, kills: 2, deaths: 5, position: { x: 20, y: 75 } },
];

export default function Spectator() {
  const { id: gameId } = useParams();
  const navigate = useNavigate();
  const wsRef = useRef(null);

  const [players, setPlayers] = useState(MOCK_PLAYERS);
  const [cameraMode, setCameraMode] = useState('follow');
  const [followedPlayer, setFollowedPlayer] = useState(MOCK_PLAYERS[0]);
  const [spectators, setSpectators] = useState(MOCK_SPECTATORS);
  const [chatMessages, setChatMessages] = useState([
    { id: 1, player: 'Viewer123', message: 'Great game!', timestamp: 60000 },
    { id: 2, player: 'GamerFan', message: 'This is intense', timestamp: 30000 },
  ]);
  const [chatInput, setChatInput] = useState('');
  const [cameraPosition, setCameraPosition] = useState({ x: 0, y: 0 });

  useEffect(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/spectate/${gameId}`);
    wsRef.current = ws;

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: 'join_spectate', gameId }));
    };

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      switch (data.type) {
        case 'players_update':
          setPlayers(data.players);
          if (cameraMode === 'follow' && followedPlayer) {
            const updated = data.players.find(p => p.id === followedPlayer.id);
            if (updated) setFollowedPlayer(updated);
          }
          break;
        case 'spectator_update':
          setSpectators(data.spectators);
          break;
        case 'chat_message':
          setChatMessages(prev => [...prev, data.message]);
          break;
      }
    };

    ws.onclose = () => {
      navigate('/matchmaking');
    };

    return () => ws.close();
  }, [gameId, cameraMode, followedPlayer, navigate]);

  useEffect(() => {
    if (cameraMode === 'follow' && followedPlayer) {
      setCameraPosition({ x: followedPlayer.position.x, y: followedPlayer.position.y });
    }
  }, [cameraMode, followedPlayer]);

  const handleSendChat = (e) => {
    e.preventDefault();
    if (!chatInput.trim()) return;

    const newMessage = {
      id: Date.now(),
      player: 'You',
      message: chatInput,
      timestamp: Date.now(),
    };
    setChatMessages(prev => [...prev, newMessage]);

    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'spectator_chat', message: chatInput }));
    }

    setChatInput('');
  };

  const handleExit = () => {
    if (wsRef.current) {
      wsRef.current.send(JSON.stringify({ type: 'leave_spectate' }));
      wsRef.current.close();
    }
    navigate('/matchmaking');
  };

  return (
    <div className="spectator-container">
      <div className="spectator-viewport">
        <div
          className="game-world"
          style={{
            transform: cameraMode === 'follow'
              ? `translate(${-followedPlayer?.position.x}%, ${-followedPlayer?.position.y}%)`
              : `translate(${cameraPosition.x}px, ${cameraPosition.y}px)`,
          }}
        >
          <div className="game-map">
            {players.map(player => (
              <div
                key={player.id}
                className={`player-marker ${cameraMode === 'follow' && followedPlayer?.id === player.id ? 'followed' : ''}`}
                style={{
                  left: `${player.position.x}%`,
                  top: `${player.position.y}%`,
                }}
              >
                <div className="player-icon" />
                <span className="player-name">{player.username}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="spectator-overlay">
        <div className="spectator-header">
          <div className="camera-mode-selector">
            <button
              className={`mode-btn ${cameraMode === 'follow' ? 'active' : ''}`}
              onClick={() => setCameraMode('follow')}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="2" />
                <path d="M8 1v2M8 13v2M1 8h2M13 8h2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              Follow
            </button>
            <button
              className={`mode-btn ${cameraMode === 'free' ? 'active' : ''}`}
              onClick={() => setCameraMode('free')}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <rect x="2" y="2" width="12" height="12" rx="2" stroke="currentColor" strokeWidth="2" />
                <path d="M8 5v6M5 8h6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              Free Camera
            </button>
          </div>

          <div className="spectators-count">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="6" r="3" stroke="currentColor" strokeWidth="2" />
              <path d="M2 14c0-3 2.5-5 6-5s6 2 6 5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
            <span>{spectators.length} watching</span>
          </div>

          <button className="exit-btn" onClick={handleExit}>
            Exit Spectator
          </button>
        </div>

        {cameraMode === 'follow' && (
          <div className="followed-player-info">
            <span className="followed-label">Following</span>
            <select
              value={followedPlayer?.id || ''}
              onChange={(e) => {
                const player = players.find(p => p.id === parseInt(e.target.value));
                setFollowedPlayer(player);
              }}
              className="player-select"
            >
              {players.map(player => (
                <option key={player.id} value={player.id}>
                  {player.username} ({player.score} pts)
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="leaderboard-sidebar">
          <h3>Players</h3>
          <div className="players-list">
            {players.map((player, index) => (
              <div
                key={player.id}
                className={`leaderboard-row ${cameraMode === 'follow' && followedPlayer?.id === player.id ? 'followed' : ''}`}
                onClick={() => {
                  setFollowedPlayer(player);
                  setCameraMode('follow');
                }}
              >
                <span className="rank">#{index + 1}</span>
                <span className="player-info">
                  <span className="player-name">{player.username}</span>
                  <span className="player-stats">{player.kills}K / {player.deaths}D</span>
                </span>
                <span className="player-score">{player.score}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="chat-sidebar">
          <h3>Spectator Chat</h3>
          <div className="chat-messages">
            {chatMessages.map(msg => (
              <div key={msg.id} className="chat-message">
                <span className="chat-player">{msg.player}:</span>
                <span className="chat-text">{msg.message}</span>
              </div>
            ))}
          </div>
          <form className="chat-form" onSubmit={handleSendChat}>
            <input
              type="text"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              placeholder="Send a message..."
              className="chat-input"
            />
            <button type="submit" className="chat-send-btn">Send</button>
          </form>
        </div>
      </div>

      <style>{`
        .spectator-container {
          position: fixed;
          top: 0;
          left: 0;
          width: 100vw;
          height: 100vh;
          overflow: hidden;
          background: #0a0a0f;
          display: flex;
        }
        .spectator-viewport {
          flex: 1;
          position: relative;
          overflow: hidden;
        }
        .game-world {
          position: absolute;
          width: 200%;
          height: 200%;
          transition: transform 0.1s ease-out;
        }
        .game-map {
          position: relative;
          width: 100%;
          height: 100%;
          background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
        }
        .player-marker {
          position: absolute;
          transform: translate(-50%, -50%);
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 0.25rem;
          cursor: pointer;
          z-index: 10;
        }
        .player-marker.followed {
          z-index: 20;
        }
        .player-icon {
          width: 24px;
          height: 24px;
          background: var(--color-accent, #8b5cf6);
          border-radius: 50%;
          border: 3px solid #fff;
          box-shadow: 0 0 10px rgba(139, 92, 246, 0.5);
        }
        .player-marker.followed .player-icon {
          box-shadow: 0 0 20px rgba(139, 92, 246, 0.8);
        }
        .player-name {
          font-size: 0.75rem;
          color: #fff;
          background: rgba(0, 0, 0, 0.6);
          padding: 0.125rem 0.5rem;
          border-radius: 4px;
          white-space: nowrap;
        }
        .spectator-overlay {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          pointer-events: none;
          display: flex;
          flex-direction: column;
        }
        .spectator-overlay > * {
          pointer-events: auto;
        }
        .spectator-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.5rem;
          background: linear-gradient(to bottom, rgba(0, 0, 0, 0.8), transparent);
        }
        .camera-mode-selector {
          display: flex;
          gap: 0.5rem;
        }
        .mode-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: rgba(255, 255, 255, 0.1);
          border: 1px solid rgba(255, 255, 255, 0.2);
          border-radius: var(--border-radius, 8px);
          color: #fff;
          font-size: 0.875rem;
          cursor: pointer;
          transition: all 0.2s;
        }
        .mode-btn:hover, .mode-btn.active {
          background: rgba(139, 92, 246, 0.3);
          border-color: var(--color-accent, #8b5cf6);
        }
        .spectators-count {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: rgba(0, 0, 0, 0.6);
          border-radius: var(--border-radius, 8px);
          color: #fff;
          font-size: 0.875rem;
        }
        .exit-btn {
          padding: 0.5rem 1rem;
          background: rgba(239, 68, 68, 0.2);
          border: 1px solid rgba(239, 68, 68, 0.4);
          border-radius: var(--border-radius, 8px);
          color: #ef4444;
          font-size: 0.875rem;
          cursor: pointer;
          transition: all 0.2s;
        }
        .exit-btn:hover {
          background: rgba(239, 68, 68, 0.3);
        }
        .followed-player-info {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem 1.5rem;
          background: rgba(0, 0, 0, 0.6);
        }
        .followed-label {
          font-size: 0.75rem;
          text-transform: uppercase;
          color: var(--color-text-muted, #888);
        }
        .player-select {
          padding: 0.5rem 1rem;
          background: var(--color-bg-secondary, rgba(30, 30, 40, 0.9));
          border: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
          border-radius: var(--border-radius, 8px);
          color: #fff;
          font-size: 0.875rem;
        }
        .leaderboard-sidebar {
          position: absolute;
          top: 100px;
          right: 1rem;
          width: 280px;
          background: rgba(10, 10, 15, 0.95);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .leaderboard-sidebar h3 {
          padding: 1rem;
          margin: 0;
          font-size: 0.875rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--color-text-secondary, #888);
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        .players-list {
          max-height: 300px;
          overflow-y: auto;
        }
        .leaderboard-row {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.75rem 1rem;
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
          cursor: pointer;
          transition: background 0.2s;
        }
        .leaderboard-row:hover, .leaderboard-row.followed {
          background: rgba(139, 92, 246, 0.2);
        }
        .rank {
          font-size: 0.75rem;
          font-weight: 600;
          color: var(--color-text-muted, #888);
          min-width: 24px;
        }
        .player-info {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 0.125rem;
        }
        .player-info .player-name {
          font-size: 0.875rem;
          color: #fff;
          background: transparent;
          padding: 0;
        }
        .player-stats {
          font-size: 0.625rem;
          color: var(--color-text-muted, #888);
        }
        .player-score {
          font-size: 0.875rem;
          font-weight: 600;
          color: var(--color-accent, #8b5cf6);
        }
        .chat-sidebar {
          position: absolute;
          bottom: 1rem;
          right: 1rem;
          width: 320px;
          max-height: 400px;
          background: rgba(10, 10, 15, 0.95);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }
        .chat-sidebar h3 {
          padding: 0.75rem 1rem;
          margin: 0;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--color-text-secondary, #888);
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 0.75rem;
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }
        .chat-message {
          font-size: 0.875rem;
          line-height: 1.4;
        }
        .chat-player {
          font-weight: 600;
          color: var(--color-accent, #8b5cf6);
        }
        .chat-text {
          color: #fff;
          margin-left: 0.25rem;
        }
        .chat-form {
          display: flex;
          gap: 0.5rem;
          padding: 0.75rem;
          border-top: 1px solid rgba(255, 255, 255, 0.1);
        }
        .chat-input {
          flex: 1;
          padding: 0.5rem 0.75rem;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 4px);
          color: #fff;
          font-size: 0.875rem;
        }
        .chat-input:focus {
          outline: none;
          border-color: var(--color-accent, #8b5cf6);
        }
        .chat-send-btn {
          padding: 0.5rem 1rem;
          background: var(--color-accent, #8b5cf6);
          border: none;
          border-radius: var(--border-radius, 4px);
          color: #fff;
          font-size: 0.875rem;
          cursor: pointer;
          transition: background 0.2s;
        }
        .chat-send-btn:hover {
          background: #7c3aed;
        }
      `}</style>
    </div>
  );
}
