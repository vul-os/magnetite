import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import GameHUD from '../components/GameHUD';
import Modal from '../components/Modal';

const MOCK_PLAYERS = [
  { id: 1, username: 'PlayerOne', score: 1250, kills: 5, deaths: 2, ping: 24 },
  { id: 2, username: 'GameMaster', score: 1100, kills: 4, deaths: 3, ping: 31 },
  { id: 3, username: 'ProGamer99', score: 980, kills: 3, deaths: 4, ping: 18 },
  { id: 4, username: 'NoobMaster', score: 720, kills: 2, deaths: 5, ping: 45 },
];

export default function Playground() {
  const { id: gameId } = useParams();
  const navigate = useNavigate();
  const canvasRef = useRef(null);
  const wsRef = useRef(null);
  const gameLoopRef = useRef(null);

  const [connectionStatus, setConnectionStatus] = useState('disconnected');
  const [latency, setLatency] = useState(0);
  const [gameState, setGameState] = useState({
    score: 0,
    timeRemaining: 600,
    isPaused: false,
    isGameOver: false,
    winner: null,
  });
  const [players, setPlayers] = useState(MOCK_PLAYERS);
  const [showPauseMenu, setShowPauseMenu] = useState(false);
  const [chatMessages, setChatMessages] = useState([
    { id: 1, player: 'System', message: 'Game started!', timestamp: 300000 },
    { id: 2, player: 'PlayerOne', message: 'Good luck everyone!', timestamp: 240000 },
  ]);
  const [minimapData, setMinimapData] = useState({ players: [], objectives: [] });

  const connectWebSocket = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/game/${gameId}`);
    wsRef.current = ws;

    ws.onopen = () => {
      setConnectionStatus('connected');
      ws.send(JSON.stringify({ type: 'join_game', playerId: 1 }));
    };

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      switch (data.type) {
        case 'game_state':
          setGameState(data.state);
          setPlayers(data.players);
          break;
        case 'player_update':
          setPlayers(prev => prev.map(p => p.id === data.player.id ? data.player : p));
          break;
        case 'minimap_update':
          setMinimapData(data);
          break;
        case 'chat_message':
          setChatMessages(prev => [...prev, data.message]);
          break;
        case 'pong':
          setLatency(Date.now() - data.timestamp);
          break;
      }
    };

    ws.onclose = () => {
      setConnectionStatus('disconnected');
    };

    ws.onerror = () => {
      setConnectionStatus('error');
    };

    return ws;
  }, [gameId]);

  useEffect(() => {
    const ws = connectWebSocket();

    const pingInterval = setInterval(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: 'ping', timestamp: Date.now() }));
      }
    }, 5000);

    return () => {
      ws.close();
      clearInterval(pingInterval);
    };
  }, [connectWebSocket]);

  useEffect(() => {
    const initCanvas = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;

      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#1a1a2e';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      for (let i = 0; i < 50; i++) {
        ctx.beginPath();
        ctx.arc(
          Math.random() * canvas.width,
          Math.random() * canvas.height,
          Math.random() * 3 + 1,
          0,
          Math.PI * 2
        );
        ctx.fillStyle = `rgba(255, 255, 255, ${Math.random() * 0.5 + 0.2})`;
        ctx.fill();
      }
    };

    initCanvas();
    window.addEventListener('resize', initCanvas);

    return () => window.removeEventListener('resize', initCanvas);
  }, []);

  useEffect(() => {
    const handleKeyDown = (e) => {
      if (e.key === 'Escape') {
        setShowPauseMenu(prev => !prev);
        setGameState(prev => ({ ...prev, isPaused: !prev.isPaused }));
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  useEffect(() => {
    if (gameState.isPaused || gameState.isGameOver) return;

    gameLoopRef.current = setInterval(() => {
      setGameState(prev => {
        if (prev.timeRemaining <= 0) {
          return { ...prev, isGameOver: true, timeRemaining: 0 };
        }
        return { ...prev, timeRemaining: prev.timeRemaining - 1 };
      });
    }, 1000);

    return () => clearInterval(gameLoopRef.current);
  }, [gameState.isPaused, gameState.isGameOver]);

  const handleSendChat = (message) => {
    const newMessage = {
      id: Date.now(),
      player: 'You',
      message,
      timestamp: Date.now(),
    };
    setChatMessages(prev => [...prev, newMessage]);

    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'chat_message', message }));
    }
  };

  const handleExit = () => {
    if (wsRef.current) {
      wsRef.current.send(JSON.stringify({ type: 'leave_game' }));
      wsRef.current.close();
    }
    navigate('/matchmaking');
  };

  const handleResume = () => {
    setShowPauseMenu(false);
    setGameState(prev => ({ ...prev, isPaused: false }));
  };

  const formatTime = (seconds) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <div className="playground-container">
      <canvas ref={canvasRef} className="game-canvas" />

      <GameHUD
        score={gameState.score}
        timeRemaining={formatTime(gameState.timeRemaining)}
        players={players}
        minimapData={minimapData}
        chatMessages={chatMessages}
        onSendChat={handleSendChat}
      />

      <div className="game-overlay-top">
        <div className="connection-status" data-status={connectionStatus}>
          <span className="status-dot" />
          <span className="status-text">
            {connectionStatus === 'connected' ? 'Connected' :
             connectionStatus === 'disconnected' ? 'Disconnected' : 'Error'}
          </span>
          {connectionStatus === 'connected' && (
            <span className="latency">{latency}ms</span>
          )}
        </div>

        <div className="game-timer">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="2" />
            <path d="M8 4v4l2 2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          <span>{formatTime(gameState.timeRemaining)}</span>
        </div>

        <div className="score-display">
          <span className="score-label">Score</span>
          <span className="score-value">{gameState.score}</span>
        </div>

        <button className="exit-btn" onClick={handleExit}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M6 14H3a1 1 0 01-1-1V3a1 1 0 011-1h3M10 11l3-3-3-3M6 8h7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Exit
        </button>
      </div>

      <Modal isOpen={showPauseMenu} onClose={handleResume} title="Game Paused" size="sm">
        <div className="pause-menu-content">
          <p>Game is paused</p>
          <button className="btn btn-primary" onClick={handleResume}>
            Resume
          </button>
          <button className="btn btn-secondary" onClick={handleExit}>
            Exit Game
          </button>
        </div>
      </Modal>

      <Modal isOpen={gameState.isGameOver} onClose={() => {}} title="Game Over" size="sm" closeOnBackdrop={false} closeOnEscape={false} showCloseButton={false}>
        <div className="game-over-content">
          <div className="winner-announcement">
            {gameState.winner ? `${gameState.winner} wins!` : 'Game Over!'}
          </div>
          <div className="final-score">
            <span>Final Score: {gameState.score}</span>
          </div>
          <button className="btn btn-primary" onClick={handleExit}>
            Back to Menu
          </button>
        </div>
      </Modal>

      <style>{`
        .playground-container {
          position: fixed;
          top: 0;
          left: 0;
          width: 100vw;
          height: 100vh;
          overflow: hidden;
          background: #0a0a0f;
        }
        .game-canvas {
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
        }
        .game-overlay-top {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.5rem;
          background: linear-gradient(to bottom, rgba(0, 0, 0, 0.7), transparent);
          pointer-events: none;
        }
        .game-overlay-top > * {
          pointer-events: auto;
        }
        .connection-status {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: var(--color-bg-secondary, rgba(20, 20, 30, 0.9));
          border-radius: var(--border-radius, 8px);
          font-size: 0.875rem;
        }
        .status-dot {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background: var(--color-text-muted, #666);
        }
        .connection-status[data-status="connected"] .status-dot {
          background: var(--color-success, #22c55e);
        }
        .connection-status[data-status="error"] .status-dot {
          background: var(--color-error, #ef4444);
        }
        .status-text {
          color: var(--color-text-primary, #fff);
        }
        .latency {
          color: var(--color-text-muted, #888);
          font-size: 0.75rem;
          margin-left: 0.5rem;
        }
        .game-timer {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: var(--color-bg-secondary, rgba(20, 20, 30, 0.9));
          border-radius: var(--border-radius, 8px);
          color: var(--color-text-primary, #fff);
          font-size: 1.25rem;
          font-weight: 600;
          font-variant-numeric: tabular-nums;
        }
        .score-display {
          display: flex;
          flex-direction: column;
          align-items: center;
          padding: 0.5rem 1.5rem;
          background: var(--color-bg-secondary, rgba(20, 20, 30, 0.9));
          border-radius: var(--border-radius, 8px);
        }
        .score-label {
          font-size: 0.625rem;
          text-transform: uppercase;
          color: var(--color-text-muted, #888);
          letter-spacing: 0.1em;
        }
        .score-value {
          font-size: 1.5rem;
          font-weight: 700;
          color: var(--color-accent, #8b5cf6);
        }
        .exit-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
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
        .pause-menu-content, .game-over-content {
          display: flex;
          flex-direction: column;
          gap: 1rem;
          text-align: center;
        }
        .pause-menu-content p {
          color: var(--color-text-secondary, #888);
        }
        .winner-announcement {
          font-size: 1.5rem;
          font-weight: 700;
          color: var(--color-accent, #8b5cf6);
        }
        .final-score {
          color: var(--color-text-secondary, #888);
        }
      `}</style>
    </div>
  );
}
