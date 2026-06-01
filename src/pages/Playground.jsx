import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import GameHUD from '../components/GameHUD';
import Modal from '../components/Modal';
import GameOverlay from '../components/GameOverlay';
import InGameStore from '../components/store/InGameStore';
import { useAuth } from '../hooks/useAuth';
import { useComms } from '../context/CommsContext';
import { usePoints } from '../hooks/usePoints';
import { usePlayManifest } from '../hooks/usePlayManifest';
import './Playground.css';

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

  // Auth + economy
  const { user }                    = useAuth();
  const comms                       = useComms();
  const { balance }                 = usePoints();
  const [showStore, setShowStore]   = useState(false);
  // Stable ref for user id so connectWebSocket doesn't re-mount on user changes
  const userIdRef = useRef(null);
  userIdRef.current = user?.id ?? null;

  // ── Play manifest — resolve live ws_endpoint from the distribution API ────
  const {
    manifest,
    loading: manifestLoading,
    error: manifestError,
    reload: reloadManifest,
  } = usePlayManifest(gameId);

  const [connectionStatus, setConnectionStatus] = useState('disconnected');
  const [latency, setLatency]                   = useState(0);
  const [gameState, setGameState]               = useState({
    score: 0,
    timeRemaining: 600,
    isPaused: false,
    isGameOver: false,
    winner: null,
  });
  // Players are populated from real WS game_state messages
  const [players, setPlayers]         = useState([]);
  const [showPauseMenu, setShowPauseMenu] = useState(false);
  const [chatMessages, setChatMessages] = useState([]);
  const [minimapData] = useState({ players: [], objectives: [] });

  const connectWebSocket = useCallback((wsEndpoint) => {
    // Use the live endpoint from the play manifest; fall back to the path-based URL
    // derived from the current host so local dev without a provisioned instance still works.
    let wsUrl;
    if (wsEndpoint) {
      wsUrl = wsEndpoint;
    } else {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      wsUrl = `${protocol}//${window.location.host}/ws/game/${gameId}`;
    }

    const ws        = new WebSocket(wsUrl);
    wsRef.current   = ws;

    ws.onopen = () => {
      setConnectionStatus('connected');
      ws.send(JSON.stringify({ type: 'join_game', playerId: userIdRef.current }));
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

  // Wait for the manifest before opening the WebSocket so we connect to the
  // provisioned server URL rather than any hardcoded default.
  useEffect(() => {
    // While the manifest is still loading, don't open the socket yet.
    if (manifestLoading) return;
    // If the manifest failed and there is no server_url, we still attempt a
    // connection using the fallback URL so the game page stays functional.
    const wsEndpoint = manifest?.server_url ?? null;
    const ws = connectWebSocket(wsEndpoint);

    const pingInterval = setInterval(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: 'ping', timestamp: Date.now() }));
      }
    }, 5000);

    return () => {
      ws.close();
      clearInterval(pingInterval);
    };
  }, [connectWebSocket, manifestLoading, manifest]);

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

  // ── Manifest loading / error gates ────────────────────────────────────────
  if (manifestLoading) {
    return (
      <div className="playground-container playground-loading" role="main" aria-label="Loading game">
        <div className="playground-status-card" aria-live="polite">
          <span className="playground-status-kicker">// Connecting</span>
          <p className="playground-status-msg">Fetching game server&hellip;</p>
        </div>
      </div>
    );
  }

  if (manifestError && !manifest) {
    return (
      <div className="playground-container playground-error" role="main" aria-label="Game unavailable">
        <div className="playground-status-card" role="alert">
          <span className="playground-status-kicker">// Error</span>
          <p className="playground-status-msg">{manifestError}</p>
          <button className="btn btn-primary" onClick={reloadManifest}>Retry</button>
          <button className="btn btn-secondary" onClick={() => navigate('/matchmaking')}>Back</button>
        </div>
      </div>
    );
  }

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

        {/* Points HUD */}
        <div className="points-hud" aria-label={`Points balance: ${balance.points}`}>
          <span className="points-hud-icon" aria-hidden="true">⬡</span>
          <span className="points-hud-value">{(balance.points ?? 0).toLocaleString()}</span>
          <span className="points-hud-label">pts</span>
        </div>

        {/* Player badge */}
        {user && (
          <div className="player-hud" aria-label={`Signed in as ${user.username ?? user.email}`}>
            <span className="player-hud-avatar" aria-hidden="true">
              {(user.username ?? user.email ?? 'P').charAt(0).toUpperCase()}
            </span>
            <span className="player-hud-name">{user.username ?? user.email}</span>
          </div>
        )}

        {/* In-game store toggle */}
        <button
          className="store-hud-btn"
          onClick={() => setShowStore((v) => !v)}
          aria-expanded={showStore}
          aria-label="Toggle in-game store"
          title="Store"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z" />
            <line x1="3" x2="21" y1="6" y2="6" />
            <path d="M16 10a4 4 0 0 1-8 0" />
          </svg>
          Store
        </button>

        <button className="exit-btn" onClick={handleExit} aria-label="Exit game">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M6 14H3a1 1 0 01-1-1V3a1 1 0 011-1h3M10 11l3-3-3-3M6 8h7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Exit
        </button>
      </div>

      {/* In-game store panel */}
      {showStore && (
        <div className="ingame-store-overlay" role="region" aria-label="In-game store">
          <InGameStore
            storeId={gameId ? `game-${gameId}` : undefined}
            gameTitle="Match Store"
            onClose={() => setShowStore(false)}
            pointBalance={balance.points ?? 0}
          />
        </div>
      )}

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

      {/* In-game comms overlay — chat + voice (Tab / ` to toggle) */}
      <GameOverlay
        label="Match Chat"
        channelId={gameId ? `match-${gameId}` : null}
        voiceRoomId={gameId ? `match-voice-${gameId}` : null}
        comms={comms}
      />
    </div>
  );
}
