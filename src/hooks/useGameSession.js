import { useState, useEffect, useCallback } from 'react';
import { useWebSocket } from './useWebSocket';

export function useGameSession(gameId) {
  const { isConnected, lastMessage, sendMessage, reconnect } = useWebSocket(`/ws/game/${gameId}`);

  const [gameState, setGameState] = useState(null);
  const [players, setPlayers] = useState([]);
  const [sessionStatus, setSessionStatus] = useState('connecting');
  const [sessionError, setSessionError] = useState(null);

  useEffect(() => {
    if (!gameId) {
      setSessionStatus('invalid');
      return;
    }
    setSessionStatus('connecting');
  }, [gameId]);

  // Transition to 'active' once the WS opens and request initial state
  useEffect(() => {
    if (isConnected && sessionStatus === 'connecting') {
      setSessionStatus('active');
      sendMessage({ type: 'join_game', gameId });
    }
  }, [isConnected, sessionStatus, sendMessage, gameId]);

  useEffect(() => {
    if (lastMessage?.type === 'game_state' || lastMessage?.type === 'game_state_update') {
      if (lastMessage.state) setGameState(lastMessage.state);
      if (Array.isArray(lastMessage.players)) setPlayers(lastMessage.players);
    }
    if (lastMessage?.type === 'player_joined') {
      setPlayers((p) => [...p, lastMessage.player]);
    }
    if (lastMessage?.type === 'player_left') {
      setPlayers((p) => p.filter((pl) => pl.id !== lastMessage.playerId));
    }
    if (lastMessage?.type === 'game_over') {
      setSessionStatus('finished');
      setGameState(lastMessage.finalState);
    }
    if (lastMessage?.type === 'error') {
      setSessionError(lastMessage.message);
    }
  }, [lastMessage]);

  const makeMove = useCallback((cellIndex) => {
    sendMessage({ type: 'make_move', cellIndex, gameId });
  }, [sendMessage, gameId]);

  const leaveSession = useCallback(() => {
    sendMessage({ type: 'leave_session', gameId });
    setSessionStatus('left');
  }, [sendMessage, gameId]);

  const getSessionInfo = useCallback(() => ({
    gameId,
    status: sessionStatus,
    error: sessionError,
    isConnected,
    playerCount: players.length,
  }), [gameId, sessionStatus, sessionError, isConnected, players.length]);

  return {
    gameState,
    players,
    sessionStatus,
    makeMove,
    leaveSession,
    getSessionInfo,
    reconnect,
  };
}
