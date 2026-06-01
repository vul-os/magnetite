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

  // Transition to 'active' once the WS opens.
  // Backend has no 'join_game' variant — PlayerJoin is the server-emitted event,
  // not a client command. Just mark as active once connected.
  useEffect(() => {
    if (isConnected && sessionStatus === 'connecting') {
      setSessionStatus('active');
    }
  }, [isConnected, sessionStatus]);

  useEffect(() => {
    // Backend GameMessage with rename_all="snake_case" emits:
    //   state_update  { state: GameState }
    //   player_join   { player_id }
    //   player_leave  { player_id }
    //   chat          { player_id, message }
    // Keep legacy aliases (game_state, game_state_update) for compatibility
    // with any older backend build until the rename lands.
    const t = lastMessage?.type;
    if (t === 'state_update' || t === 'game_state' || t === 'game_state_update') {
      if (lastMessage.state) setGameState(lastMessage.state);
      if (Array.isArray(lastMessage.players)) setPlayers(lastMessage.players);
    }
    if (t === 'player_join' || t === 'player_joined') {
      const player = lastMessage.player ?? { id: lastMessage.player_id };
      setPlayers((p) => [...p, player]);
    }
    if (t === 'player_leave' || t === 'player_left') {
      const id = lastMessage.player_id ?? lastMessage.playerId;
      setPlayers((p) => p.filter((pl) => pl.id !== id));
    }
    if (t === 'game_over') {
      setSessionStatus('finished');
      setGameState(lastMessage.finalState ?? lastMessage.state ?? null);
    }
    if (t === 'error') {
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
