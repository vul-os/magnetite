import { useState, useCallback, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import PlayerList from '../components/PlayerList';
import LobbyChat from '../components/LobbyChat';
import ReadyButton from '../components/ReadyButton';
import StartGameButton from '../components/StartGameButton';
import GameOverlay from '../components/GameOverlay';
import InGameStore from '../components/store/InGameStore';
import { useAuth } from '../hooks/useAuth';
import { useComms } from '../context/CommsContext';
import { usePoints } from '../hooks/usePoints';
import { useGameLobby } from '../hooks/useGameLobby';
import { usePlayManifest } from '../hooks/usePlayManifest';
import { useTranslation } from '../i18n/useTranslation';
import './GameLobby.css';

export default function GameLobby() {
  const { t } = useTranslation();
  const { id: _gameId } = useParams();
  const navigate        = useNavigate();

  // Real user from auth; fall back to a guest stub for unauthenticated dev use.
  const { user }     = useAuth();
  const comms        = useComms();
  const { balance }  = usePoints();

  const currentUser = user
    ? { id: String(user.id), username: user.username ?? user.email ?? 'You', avatar: user.avatar_url ?? null }
    : { id: 'guest', username: 'Guest', avatar: null };

  const [showStore, setShowStore] = useState(false);

  // ── Play manifest — resolve live ws_endpoint ahead of game start ──────────
  const {
    manifest: playManifest,
    loading: manifestLoading,
    error: manifestError,
  } = usePlayManifest(_gameId);

  // Connect to the real lobby WebSocket via useGameLobby
  const {
    players,
    chatMessages,
    lobbyState,
    countdown,
    isHost,
    allReady,
    toggleReady,
    kickPlayer,
    sendChatMessage,
    startGame,
    isConnected,
    error: lobbyError,
  } = useGameLobby(_gameId, currentUser);

  // Determine hostId: the first host in the player list, or self if no host yet
  const hostId = players.find(p => p.isHost)?.id ?? currentUser.id;

  const currentPlayer = players.find(p => p.id === currentUser.id);

  const handleToggleReady = useCallback(() => {
    toggleReady();
  }, [toggleReady]);

  const handleKickPlayer = useCallback((playerId) => {
    kickPlayer(playerId);
  }, [kickPlayer]);

  const handleSendMessage = useCallback((content) => {
    sendChatMessage(content);
  }, [sendChatMessage]);

  const handleStartGame = useCallback(() => {
    startGame();
  }, [startGame]);

  // Navigate to the game session when the lobby transitions to 'starting'
  useEffect(() => {
    if (lobbyState === 'starting' && _gameId) {
      navigate(`/play/${_gameId}`);
    }
  }, [lobbyState, _gameId, navigate]);

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
              <span className="lobby-kicker">{t('game.lobbyKicker')}</span>
              <h1 className="lobby-title">{t('game.lobbyTitle')}</h1>
              <p className="lobby-game-name">{t('game.lobbyGame')}</p>
            </div>
            <div className="lobby-header-right">
              {/* Points HUD */}
              <div className="lobby-points-hud" aria-label={t('game.lobbyPoints', { count: balance.points ?? 0 })}>
                <span className="lobby-points-icon" aria-hidden="true">⬡</span>
                <span className="lobby-points-value">{(balance.points ?? 0).toLocaleString()}</span>
                <span className="lobby-points-label">{t('game.pointsUnit')}</span>
              </div>
              {/* In-game store toggle */}
              <button
                className="lobby-store-btn"
                onClick={() => setShowStore((v) => !v)}
                aria-expanded={showStore}
                aria-label={t('game.toggleStore')}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z" />
                  <line x1="3" x2="21" y1="6" y2="6" />
                  <path d="M16 10a4 4 0 0 1-8 0" />
                </svg>
                {t('game.storeLabel')}
              </button>
              <div className="lobby-code-block">
                <span className="lobby-code-label">{t('game.lobbyCode')}</span>
                <span className="lobby-code-value" aria-label={t('game.lobbyCodeLabel', { code: _gameId ?? 'unknown' })}>
                  {_gameId ?? '—'}
                </span>
              </div>
              <div
                className={`lobby-conn-status ${isConnected ? 'connected' : 'disconnected'}`}
                aria-live="polite"
                aria-label={isConnected ? t('game.connectedLabel') : t('game.connectingLabel')}
              >
                <span className="status-dot" aria-hidden="true" />
                {isConnected ? t('game.connected') : t('game.connecting')}
              </div>

              {/* Game server status — resolved from the play manifest */}
              <div
                className={`lobby-server-status ${
                  manifestLoading ? 'pending' :
                  manifestError   ? 'error'   :
                  playManifest?.server_url ? 'ready' : 'pending'
                }`}
                aria-live="polite"
                aria-label={
                  manifestLoading ? t('game.serverResolving') :
                  manifestError   ? t('game.serverUnavailable', { error: manifestError }) :
                  playManifest?.server_url ? t('game.serverReady') : t('game.serverPending')
                }
              >
                <span className="status-dot" aria-hidden="true" />
                {manifestLoading           ? t('game.serverStatus')    :
                 manifestError             ? t('game.serverNoServer') :
                 playManifest?.server_url  ? t('game.serverReady') : t('game.serverPending')}
              </div>
            </div>
          </header>

          {/* Error banner */}
          {lobbyError && (
            <div className="lobby-error-banner" role="alert">
              {lobbyError}
            </div>
          )}

          {/* Countdown overlay */}
          {countdown !== null && (
            <div className="lobby-countdown" role="status" aria-live="assertive">
              <span className="countdown-label">{t('game.startingIn')}</span>
              <span className="countdown-value">{countdown}</span>
            </div>
          )}

          {/* In-game store panel */}
          {showStore && (
            <div className="lobby-store-panel" role="region" aria-label={t('game.storeRegion')}>
              <InGameStore
                storeId={_gameId ? `game-${_gameId}` : undefined}
                gameTitle="Lobby Store"
                onClose={() => setShowStore(false)}
                pointBalance={balance.points ?? 0}
              />
            </div>
          )}

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
                <h3>{t('game.readyStatus')}</h3>
                <ReadyButton
                  isReady={currentPlayer?.isReady || false}
                  isHost={isHost}
                  onToggleReady={handleToggleReady}
                />
              </div>

              {isHost && (
                <div className="lobby-section-card">
                  <h3>{t('game.hostControls')}</h3>
                  <StartGameButton
                    allReady={allReady}
                    playerCount={players.length}
                    minPlayers={2}
                    onStartGame={handleStartGame}
                  />
                </div>
              )}

              {!isHost && !allReady && (
                <div className="waiting-state" aria-live="polite">
                  <div className="waiting-dot" aria-hidden="true" />
                  {t('game.waitingForPlayers')}
                </div>
              )}
            </div>

            {/* Chat */}
            <div className="lobby-chat-col">
              <LobbyChat
                messages={chatMessages}
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
        comms={comms}
      />
    </div>
  );
}
