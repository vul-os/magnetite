import { useState, useEffect, useCallback, useRef } from 'react';
import { useWebSocket } from './useWebSocket';

export interface LobbyPlayer {
  id: string;
  isHost?: boolean;
  isReady?: boolean;
  [key: string]: unknown;
}

export interface LobbyChatMessage {
  id: number;
  playerId: string;
  username?: string | undefined;
  message: string;
  timestamp: string;
}

export interface LobbyUser {
  id: string;
  username?: string;
  [key: string]: unknown;
}

export type LobbyState = 'connecting' | 'invalid' | 'waiting' | 'starting' | 'left' | string;

export function useGameLobby(lobbyId: string | null | undefined, currentUser: LobbyUser | null | undefined) {
  // /ws/lobby/:id has no backend handler (AUDIT critical).
  // Route lobby traffic over /ws/game/:id — the game WS already handles
  // PlayerJoin/Chat/StateUpdate and useWebSocket will append ?token=<jwt>.
  const { isConnected, lastMessage, sendMessage, reconnect } = useWebSocket(`/ws/game/${lobbyId}`);

  const [players, setPlayers] = useState<LobbyPlayer[]>([]);
  const [chatMessages, setChatMessages] = useState<LobbyChatMessage[]>([]);
  const [lobbyState, setLobbyState] = useState<LobbyState>('connecting');
  const [countdown, setCountdown] = useState<number | null>(null);
  const [gameRules, setGameRules] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const countdownRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const isHost = players.find(p => p.id === currentUser?.id)?.isHost || false;
  const allReady = players.length > 0 && players.every(p => p.isReady || p.isHost);

  // When invalid lobby/user, mark as invalid immediately
  useEffect(() => {
    if (!lobbyId || !currentUser) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setLobbyState('invalid');
    }
  }, [lobbyId, currentUser]);

  // Transition from 'connecting' to 'waiting' once the WS is live
  useEffect(() => {
    if (isConnected && lobbyState === 'connecting') {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setLobbyState('waiting');
      // Request current lobby state from the server
      sendMessage({ type: 'get_lobby_state' });
    }
  }, [isConnected, lobbyState, sendMessage]);

  useEffect(() => {
    return () => {
      if (countdownRef.current) clearInterval(countdownRef.current);
    };
  }, []);

  // Inbound WebSocket frames (external system) drive lobby state below.
  useEffect(() => {
    if (!lastMessage || typeof lastMessage === 'string') return;
    const msg = lastMessage;

    switch (msg.type) {
      case 'lobby_state': {
        // Full lobby snapshot sent on join
        // eslint-disable-next-line react-hooks/set-state-in-effect
        if (Array.isArray(msg.players)) setPlayers(msg.players as LobbyPlayer[]);
        if (msg.rules) setGameRules(msg.rules);
        if (msg.state) setLobbyState(msg.state as LobbyState);
        break;
      }
      case 'player_joined': {
        const player = msg.player as LobbyPlayer;
        if (!players.find(p => p.id === player.id)) {
          setPlayers(prev => [...prev, player]);
        }
        break;
      }
      case 'player_left':
        setPlayers(prev => prev.filter(p => p.id !== msg.playerId));
        break;
      case 'player_ready':
        setPlayers(prev =>
          prev.map(p => p.id === msg.playerId ? { ...p, isReady: msg.isReady as boolean } : p)
        );
        break;
      case 'player_kicked':
        setPlayers(prev => prev.filter(p => p.id !== msg.playerId));
        break;
      case 'chat_message':
        setChatMessages(prev => [...prev, msg.message as LobbyChatMessage]);
        break;
      case 'lobby_state_update':
        setLobbyState(msg.state as LobbyState);
        break;
      case 'countdown_start':
        setCountdown(msg.seconds as number);
        countdownRef.current = setInterval(() => {
          setCountdown(prev => {
            if (prev == null || prev <= 1) {
              if (countdownRef.current) clearInterval(countdownRef.current);
              return null;
            }
            return prev - 1;
          });
        }, 1000);
        break;
      case 'countdown_cancel':
        if (countdownRef.current) clearInterval(countdownRef.current);
        setCountdown(null);
        break;
      case 'game_start':
        setLobbyState('starting');
        break;
      case 'error':
        setError(msg.message as string);
        break;
    }
  }, [lastMessage, players]);

  const toggleReady = useCallback(() => {
    const player = players.find(p => p.id === currentUser?.id);
    if (!player || !currentUser) return;
    sendMessage({ type: 'toggle_ready', playerId: currentUser.id });
    setPlayers(prev =>
      prev.map(p => p.id === currentUser.id ? { ...p, isReady: !p.isReady } : p)
    );
  }, [sendMessage, currentUser, players]);

  const kickPlayer = useCallback((playerId: string) => {
    if (!isHost) return;
    sendMessage({ type: 'kick_player', playerId });
    setPlayers(prev => prev.filter(p => p.id !== playerId));
  }, [sendMessage, isHost]);

  const sendChatMessage = useCallback((message: string) => {
    if (!currentUser) return;
    const chatMsg: LobbyChatMessage = {
      id: Date.now(),
      playerId: currentUser.id,
      username: currentUser.username,
      message,
      timestamp: new Date().toISOString(),
    };
    sendMessage({ type: 'chat_message', message: chatMsg });
    setChatMessages(prev => [...prev, chatMsg]);
  }, [sendMessage, currentUser]);

  const startGame = useCallback(() => {
    if (!isHost || !allReady) return;
    sendMessage({ type: 'start_game' });
    setLobbyState('starting');
  }, [sendMessage, isHost, allReady]);

  const leaveLobby = useCallback(() => {
    sendMessage({ type: 'leave_lobby' });
    setLobbyState('left');
  }, [sendMessage]);

  const getLobbyInfo = useCallback(() => ({
    lobbyId,
    state: lobbyState,
    playerCount: players.length,
    isHost,
    allReady,
    countdown,
    error,
  }), [lobbyId, lobbyState, players.length, isHost, allReady, countdown, error]);

  return {
    players,
    chatMessages,
    lobbyState,
    countdown,
    gameRules,
    error,
    isHost,
    allReady,
    toggleReady,
    kickPlayer,
    sendChatMessage,
    startGame,
    leaveLobby,
    getLobbyInfo,
    reconnect,
    isConnected,
  };
}
