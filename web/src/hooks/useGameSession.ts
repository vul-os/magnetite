import { useState, useEffect, useCallback } from 'react';
import { useWebSocket } from './useWebSocket';

export interface SessionPlayer {
  id: string;
  [key: string]: unknown;
}

// Trailing `string` alone would swallow the 5 named literals (TS widens the
// whole union to `string`); `string & {}` keeps them distinct for
// autocomplete while still accepting any other server-sent status string.
export type SessionStatus = 'connecting' | 'invalid' | 'active' | 'finished' | 'left' | (string & {});

export function useGameSession(gameId: string | number | null | undefined) {
  const { isConnected, lastMessage, sendMessage, reconnect } = useWebSocket(`/ws/game/${gameId}`);

  const [gameState, setGameState] = useState<unknown>(null);
  const [players, setPlayers] = useState<SessionPlayer[]>([]);
  const [sessionStatus, setSessionStatus] = useState<SessionStatus>('connecting');
  const [sessionError, setSessionError] = useState<string | null>(null);

  // Reset session status when the target game changes.
  useEffect(() => {
    if (!gameId) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
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
      // eslint-disable-next-line react-hooks/set-state-in-effect
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
    // Incoming WebSocket messages (an external system) drive all of this state.
    /* eslint-disable react-hooks/set-state-in-effect */
    if (!lastMessage || typeof lastMessage === 'string') return;
    const msg = lastMessage;
    const t = msg.type;
    if (t === 'state_update' || t === 'game_state' || t === 'game_state_update') {
      if (msg.state) setGameState(msg.state);
      if (Array.isArray(msg.players)) setPlayers(msg.players as SessionPlayer[]);
    }
    if (t === 'player_join' || t === 'player_joined') {
      const player = (msg.player as SessionPlayer) ?? { id: msg.player_id as string };
      setPlayers((p) => [...p, player]);
    }
    if (t === 'player_leave' || t === 'player_left') {
      const id = msg.player_id ?? msg.playerId;
      setPlayers((p) => p.filter((pl) => pl.id !== id));
    }
    if (t === 'game_over') {
      setSessionStatus('finished');
      setGameState(msg.finalState ?? msg.state ?? null);
    }
    if (t === 'error') {
      setSessionError(msg.message as string);
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [lastMessage]);

  const makeMove = useCallback((cellIndex: number) => {
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
