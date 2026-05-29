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

    const mockInit = setTimeout(() => {
      setGameState({
        turn: 1,
        phase: 'playing',
        board: Array(9).fill(null),
        currentPlayer: 0,
      });
      setPlayers([
        { id: 1, username: 'Player1', isReady: true, isHost: true },
        { id: 2, username: 'Player2', isReady: true, isHost: false },
      ]);
      setSessionStatus('active');
    }, 500);

    return () => clearTimeout(mockInit);
  }, [gameId]);

  useEffect(() => {
    if (lastMessage?.type === 'game_state_update') {
      setGameState(lastMessage.state);
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
