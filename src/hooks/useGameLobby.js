import { useState, useEffect, useCallback, useRef } from 'react';
import { useWebSocket } from './useWebSocket';

export function useGameLobby(lobbyId, currentUser) {
  // /ws/lobby/:id has no backend handler (AUDIT critical).
  // Route lobby traffic over /ws/game/:id — the game WS already handles
  // PlayerJoin/Chat/StateUpdate and useWebSocket will append ?token=<jwt>.
  const { isConnected, lastMessage, sendMessage, reconnect } = useWebSocket(`/ws/game/${lobbyId}`);

  const [players, setPlayers] = useState([]);
  const [chatMessages, setChatMessages] = useState([]);
  const [lobbyState, setLobbyState] = useState('connecting');
  const [countdown, setCountdown] = useState(null);
  const [gameRules, setGameRules] = useState(null);
  const [error, setError] = useState(null);
  const countdownRef = useRef(null);

  const isHost = players.find(p => p.id === currentUser?.id)?.isHost || false;
  const allReady = players.length > 0 && players.every(p => p.isReady || p.isHost);

  // When invalid lobby/user, mark as invalid immediately
  useEffect(() => {
    if (!lobbyId || !currentUser) {
      setLobbyState('invalid');
    }
  }, [lobbyId, currentUser]);

  // Transition from 'connecting' to 'waiting' once the WS is live
  useEffect(() => {
    if (isConnected && lobbyState === 'connecting') {
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

  useEffect(() => {
    if (!lastMessage) return;

    switch (lastMessage.type) {
      case 'lobby_state': {
        // Full lobby snapshot sent on join
        if (Array.isArray(lastMessage.players)) setPlayers(lastMessage.players);
        if (lastMessage.rules) setGameRules(lastMessage.rules);
        if (lastMessage.state) setLobbyState(lastMessage.state);
        break;
      }
      case 'player_joined':
        if (!players.find(p => p.id === lastMessage.player.id)) {
          setPlayers(prev => [...prev, lastMessage.player]);
        }
        break;
      case 'player_left':
        setPlayers(prev => prev.filter(p => p.id !== lastMessage.playerId));
        break;
      case 'player_ready':
        setPlayers(prev =>
          prev.map(p => p.id === lastMessage.playerId ? { ...p, isReady: lastMessage.isReady } : p)
        );
        break;
      case 'player_kicked':
        setPlayers(prev => prev.filter(p => p.id !== lastMessage.playerId));
        break;
      case 'chat_message':
        setChatMessages(prev => [...prev, lastMessage.message]);
        break;
      case 'lobby_state_update':
        setLobbyState(lastMessage.state);
        break;
      case 'countdown_start':
        setCountdown(lastMessage.seconds);
        countdownRef.current = setInterval(() => {
          setCountdown(prev => {
            if (prev <= 1) {
              clearInterval(countdownRef.current);
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
        setError(lastMessage.message);
        break;
    }
  }, [lastMessage, players]);

  const toggleReady = useCallback(() => {
    const player = players.find(p => p.id === currentUser?.id);
    if (!player) return;
    sendMessage({ type: 'toggle_ready', playerId: currentUser.id });
    setPlayers(prev =>
      prev.map(p => p.id === currentUser.id ? { ...p, isReady: !p.isReady } : p)
    );
  }, [sendMessage, currentUser, players]);

  const kickPlayer = useCallback((playerId) => {
    if (!isHost) return;
    sendMessage({ type: 'kick_player', playerId });
    setPlayers(prev => prev.filter(p => p.id !== playerId));
  }, [sendMessage, isHost]);

  const sendChatMessage = useCallback((message) => {
    const chatMsg = {
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
