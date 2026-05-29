import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import GameHUD from '../components/GameHUD';
import Modal from '../components/Modal';
import './Playground.css';

const MOCK_PLAYERS = [
  { id: 1, username: 'PlayerOne',  score: 1250, kills: 5, deaths: 2, ping: 24 },
  { id: 2, username: 'GameMaster', score: 1100, kills: 4, deaths: 3, ping: 31 },
  { id: 3, username: 'ProGamer99', score:  980, kills: 3, deaths: 4, ping: 18 },
  { id: 4, username: 'NoobMaster', score:  720, kills: 2, deaths: 5, ping: 45 },
];

function formatTime(seconds) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

export default function Playground() {
  const { id: gameId } = useParams();
  const navigate        = useNavigate();
  const canvasRef       = useRef(null);
  const wsRef           = useRef(null);
  const gameLoopRef     = useRef(null);

  const [connectionStatus, setConnectionStatus] = useState('disconnected');
  const [latency, setLatency]                   = useState(0);
  const [gameState, setGameState]               = useState({
    score: 0,
    timeRemaining: 600,
    isPaused: false,
    isGameOver: false,
    winner: null,
  });
  const [players, setPlayers]         = useState(MOCK_PLAYERS);
  const [showPauseMenu, setShowPauseMenu] = useState(false);
  const [chatMessages, setChatMessages] = useState([
    { id: 1, player: 'System',    message: 'Game started!',        timestamp: 300000 },
    { id: 2, player: 'PlayerOne', message: 'Good luck everyone!',  timestamp: 240000 },
  ]);
  const [minimapData] = useState({ players: [], objectives: [] });

  const connectWebSocket = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws        = new WebSocket(`${protocol}//${window.location.host}/ws/game/${gameId}`);
    wsRef.current   = ws;

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
        case 'chat_message':
          setChatMessages(prev => [...prev, data.message]);
          break;
        case 'pong':
          setLatency(Date.now() - data.timestamp);
          break;
        default:
          break;
      }
    };

    ws.onclose = () => setConnectionStatus('disconnected');
    ws.onerror = () => setConnectionStatus('error');

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
      canvas.width  = window.innerWidth;
      canvas.height = window.innerHeight;

      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#07070b';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      // draw a faint grid (industrial motif)
      ctx.strokeStyle = 'rgba(35, 35, 46, 0.6)';
      ctx.lineWidth   = 1;
      const gridSize  = 40;
      for (let x = 0; x < canvas.width; x += gridSize) {
        ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, canvas.height); ctx.stroke();
      }
      for (let y = 0; y < canvas.height; y += gridSize) {
        ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(canvas.width, y); ctx.stroke();
      }

      // sparse star-like particles
      for (let i = 0; i < 60; i++) {
        ctx.beginPath();
        ctx.arc(
          Math.random() * canvas.width,
          Math.random() * canvas.height,
          Math.random() * 2 + 0.5,
          0, Math.PI * 2
        );
        ctx.fillStyle = `rgba(56, 225, 200, ${Math.random() * 0.25 + 0.05})`;
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
    if (gameState.isPaused || gameState.isGameOver) {
      clearInterval(gameLoopRef.current);
      return;
    }

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
    setChatMessages(prev => [
      ...prev,
      { id: Date.now(), player: 'You', message, timestamp: Date.now() },
    ]);
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

  return (
    <div className="playground-container" role="main" aria-label="Game playground">
      <canvas ref={canvasRef} className="game-canvas" aria-hidden="true" />

      <GameHUD
        score={gameState.score}
        timeRemaining={formatTime(gameState.timeRemaining)}
        players={players}
        minimapData={minimapData}
        chatMessages={chatMessages}
        onSendChat={handleSendChat}
      />

      {/* Top overlay */}
      <div className="game-overlay-top" role="toolbar" aria-label="Game controls">
        <div
          className="connection-status"
          data-status={connectionStatus}
          aria-live="polite"
          aria-label={`Connection: ${connectionStatus}`}
        >
          <span className="status-dot" aria-hidden="true" />
          <span className="status-text">
            {connectionStatus === 'connected'    ? 'Connected'    :
             connectionStatus === 'disconnected' ? 'Disconnected' : 'Error'}
          </span>
          {connectionStatus === 'connected' && (
            <span className="latency" aria-label={`Latency: ${latency} milliseconds`}>{latency}ms</span>
          )}
        </div>

        <div className="game-timer" role="timer" aria-label={`Time remaining: ${formatTime(gameState.timeRemaining)}`}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="2" />
            <path d="M8 4v4l2 2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          <span>{formatTime(gameState.timeRemaining)}</span>
        </div>

        <div className="score-display" aria-label={`Score: ${gameState.score}`}>
          <span className="score-label">Score</span>
          <span className="score-value">{gameState.score}</span>
        </div>

        <button className="exit-btn" onClick={handleExit} aria-label="Exit game">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M6 14H3a1 1 0 01-1-1V3a1 1 0 011-1h3M10 11l3-3-3-3M6 8h7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Exit
        </button>
      </div>

      {/* Pause modal */}
      <Modal isOpen={showPauseMenu} onClose={handleResume} title="// Game Paused" size="sm">
        <div className="pause-menu-content">
          <p>The game is paused. Resume or exit to the lobby.</p>
          <button className="btn btn-primary" onClick={handleResume}>Resume</button>
          <button className="btn btn-secondary" onClick={handleExit}>Exit Game</button>
        </div>
      </Modal>

      {/* Game over modal */}
      <Modal
        isOpen={gameState.isGameOver}
        onClose={() => {}}
        title="// Game Over"
        size="sm"
        closeOnBackdrop={false}
        closeOnEscape={false}
        showCloseButton={false}
      >
        <div className="game-over-content">
          <div className="winner-announcement">
            {gameState.winner ? `${gameState.winner} wins!` : 'Match Complete'}
          </div>
          <div className="final-score">
            Final Score: {gameState.score}
          </div>
          <button className="btn btn-primary" onClick={handleExit}>
            Back to Matchmaking
          </button>
        </div>
      </Modal>
    </div>
  );
}
