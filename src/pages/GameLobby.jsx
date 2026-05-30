import { useState, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import PlayerList from '../components/PlayerList';
import LobbyChat from '../components/LobbyChat';
import ReadyButton from '../components/ReadyButton';
import StartGameButton from '../components/StartGameButton';
import GameOverlay from '../components/GameOverlay';
import './GameLobby.css';

const MOCK_CURRENT_USER = {
  id: 'user-1',
  username: 'PlayerOne',
  avatar: null,
};

const MOCK_PLAYERS = [
  { id: 'user-1', username: 'PlayerOne',    avatar: null, ready: false },
  { id: 'user-2', username: 'ChessMaster99', avatar: null, ready: true },
  { id: 'user-3', username: 'GoPlayer42',    avatar: null, ready: true },
  { id: 'user-4', username: 'CardShark',     avatar: null, ready: false },
];

const MOCK_MESSAGES = [
  { id: 1, senderId: 'user-2', senderName: 'ChessMaster99', content: 'Hey everyone! Ready to play?',         timestamp: Date.now() - 120000 },
  { id: 2, senderId: 'user-3', senderName: 'GoPlayer42',    content: 'Ready when you are!',                  timestamp: Date.now() -  90000 },
  { id: 3, senderId: 'user-4', senderName: 'CardShark',     content: 'Give me a minute to finish this hand', timestamp: Date.now() -  60000 },
];

export default function GameLobby() {
  const { id: _gameId } = useParams();
  const [players, setPlayers]   = useState(MOCK_PLAYERS);
  const [messages, setMessages] = useState(MOCK_MESSAGES);
  const [hostId]                = useState('user-1');
  const currentUser              = MOCK_CURRENT_USER;

  const isHost         = currentUser.id === hostId;
  const allPlayersReady = players.every(p => p.ready);
  const currentPlayer   = players.find(p => p.id === currentUser.id);

  const handleToggleReady = useCallback((ready) => {
    setPlayers(prev => prev.map(p => p.id === currentUser.id ? { ...p, ready } : p));
  }, [currentUser.id]);

  const handleKickPlayer = useCallback((playerId) => {
    setPlayers(prev => prev.filter(p => p.id !== playerId));
  }, []);

  const handleSendMessage = useCallback((content) => {
    const newMsg = {
      id: Date.now(),
      senderId: currentUser.id,
      senderName: currentUser.username,
      content,
      timestamp: Date.now(),
    };
    setMessages(prev => [...prev, newMsg]);
  }, [currentUser.id, currentUser.username]);

  const handleStartGame = useCallback(() => {
    console.log('Starting game...');
  }, []);

  // Derive a stable channel + voice room from the game/lobby id
  const overlayChannelId   = _gameId ? `lobby-${_gameId}` : 'lobby-default';
  const overlayVoiceRoomId = _gameId ? `lobby-voice-${_gameId}` : null;

  return (
    <div className="game-lobby" role="main">
      <div className="lobby-container">
        <div className="lobby-main">
          {/* ── Header ── */}
          <header className="lobby-header">
            <div className="lobby-info-group">
              <span className="lobby-kicker">// Game Lobby</span>
              <h1 className="lobby-title">Rust Match</h1>
              <p className="lobby-game-name">Chess — Server-Authoritative</p>
            </div>
            <div className="lobby-code-block">
              <span className="lobby-code-label">// Lobby Code</span>
              <span className="lobby-code-value" aria-label="Lobby code XKCD42">XKCD42</span>
            </div>
          </header>

          {/* ── Three-column layout ── */}
          <div className="lobby-content">
            {/* Player list */}
            <div className="lobby-sidebar">
              <PlayerList
                players={players}
                hostId={hostId}
                currentUserId={currentUser.id}
                onKickPlayer={isHost ? handleKickPlayer : undefined}
              />
            </div>

            {/* Center controls */}
            <div className="lobby-center">
              <div className="lobby-section-card">
                <h3>// Ready Status</h3>
                <ReadyButton
                  isReady={currentPlayer?.ready || false}
                  isHost={isHost}
                  onToggleReady={handleToggleReady}
                />
              </div>

              {isHost && (
                <div className="lobby-section-card">
                  <h3>// Host Controls</h3>
                  <StartGameButton
                    allPlayersReady={allPlayersReady}
                    playerCount={players.length}
                    minPlayers={2}
                    onStartGame={handleStartGame}
                  />
                </div>
              )}

              {!isHost && !allPlayersReady && (
                <div className="waiting-state" aria-live="polite">
                  <div className="waiting-dot" aria-hidden="true" />
                  Waiting for all players to ready up…
                </div>
              )}
            </div>

            {/* Chat */}
            <div className="lobby-chat-col">
              <LobbyChat
                messages={messages}
                currentUserId={currentUser.id}
                onSendMessage={handleSendMessage}
              />
            </div>
          </div>
        </div>
      </div>

      {/* In-game comms overlay — chat + voice (Tab / ` to toggle) */}
      <GameOverlay
        label="Lobby"
        channelId={overlayChannelId}
        voiceRoomId={overlayVoiceRoomId}
      />
    </div>
  );
}
