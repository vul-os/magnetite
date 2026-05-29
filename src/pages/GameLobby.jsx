import { useState, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import PlayerList from '../components/PlayerList';
import LobbyChat from '../components/LobbyChat';
import ReadyButton from '../components/ReadyButton';
import StartGameButton from '../components/StartGameButton';

const MOCK_CURRENT_USER = {
  id: 'user-1',
  username: 'PlayerOne',
  avatar: null,
};

const MOCK_PLAYERS = [
  { id: 'user-1', username: 'PlayerOne', avatar: null, ready: false },
  { id: 'user-2', username: 'ChessMaster99', avatar: null, ready: true },
  { id: 'user-3', username: 'GoPlayer42', avatar: null, ready: true },
  { id: 'user-4', username: 'CardShark', avatar: null, ready: false },
];

const MOCK_MESSAGES = [
  { id: 1, senderId: 'user-2', senderName: 'ChessMaster99', content: 'Hey everyone! Ready to play?', timestamp: Date.now() - 120000 },
  { id: 2, senderId: 'user-3', senderName: 'GoPlayer42', content: 'Ready when you are!', timestamp: Date.now() - 90000 },
  { id: 3, senderId: 'user-4', senderName: 'CardShark', content: 'Give me a minute to finish this hand', timestamp: Date.now() - 60000 },
];

export default function GameLobby() {
  const { id: gameId } = useParams();
  const [players, setPlayers] = useState(MOCK_PLAYERS);
  const [messages, setMessages] = useState(MOCK_MESSAGES);
  const [hostId] = useState('user-1');
  const currentUser = MOCK_CURRENT_USER;

  const isHost = currentUser.id === hostId;
  const allPlayersReady = players.every((p) => p.ready);

  const handleToggleReady = useCallback((ready) => {
    setPlayers((prev) =>
      prev.map((p) =>
        p.id === currentUser.id ? { ...p, ready } : p
      )
    );
  }, [currentUser.id]);

  const handleKickPlayer = useCallback((playerId) => {
    setPlayers((prev) => prev.filter((p) => p.id !== playerId));
  }, []);

  const handleSendMessage = useCallback((content) => {
    const newMessage = {
      id: Date.now(),
      senderId: currentUser.id,
      senderName: currentUser.username,
      content,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, newMessage]);
  }, [currentUser]);

  const handleStartGame = useCallback(() => {
    console.log('Starting game...', { gameId, players });
  }, [gameId, players]);

  return (
    <div className="game-lobby">
      <div className="lobby-container">
        <div className="lobby-main">
          <div className="lobby-header">
            <div className="lobby-info">
              <h1>Game Lobby</h1>
              <p className="game-name">Chess Match</p>
            </div>
            <div className="lobby-code">
              <span className="code-label">Lobby Code</span>
              <span className="code-value">XKCD42</span>
            </div>
          </div>

          <div className="lobby-content">
            <div className="lobby-sidebar">
              <PlayerList
                players={players}
                hostId={hostId}
                currentUserId={currentUser.id}
                onKickPlayer={isHost ? handleKickPlayer : undefined}
              />
            </div>

            <div className="lobby-center">
              <div className="ready-section">
                <ReadyButton
                  isReady={players.find((p) => p.id === currentUser.id)?.ready || false}
                  isHost={isHost}
                  onToggleReady={handleToggleReady}
                />
              </div>

              {isHost && (
                <div className="start-section">
                  <StartGameButton
                    allPlayersReady={allPlayersReady}
                    playerCount={players.length}
                    minPlayers={2}
                    onStartGame={handleStartGame}
                  />
                </div>
              )}

              {!isHost && !allPlayersReady && (
                <div className="waiting-section">
                  <p>Waiting for host to start the game...</p>
                </div>
              )}
            </div>

            <div className="lobby-chat">
              <LobbyChat
                messages={messages}
                currentUserId={currentUser.id}
                onSendMessage={handleSendMessage}
              />
            </div>
          </div>
        </div>
      </div>

      <style>{`
        .game-lobby {
          min-height: 100vh;
          background: linear-gradient(135deg, #0a0a0f 0%, #1a1a2e 100%);
          padding: 2rem;
        }
        .lobby-container {
          max-width: 1400px;
          margin: 0 auto;
        }
        .lobby-main {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }
        .lobby-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 1.5rem 2rem;
          background: rgba(10, 10, 15, 0.8);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
        }
        .lobby-info h1 {
          margin: 0 0 0.25rem 0;
          font-size: 1.5rem;
          font-weight: 700;
          color: var(--color-text-primary, #fff);
        }
        .game-name {
          margin: 0;
          font-size: 0.875rem;
          color: var(--color-text-muted, #888);
        }
        .lobby-code {
          display: flex;
          flex-direction: column;
          align-items: flex-end;
          gap: 0.25rem;
        }
        .code-label {
          font-size: 0.625rem;
          text-transform: uppercase;
          letter-spacing: 0.1em;
          color: var(--color-text-muted, #666);
        }
        .code-value {
          font-size: 1.25rem;
          font-weight: 700;
          font-family: monospace;
          color: var(--color-accent, #8b5cf6);
          letter-spacing: 0.15em;
        }
        .lobby-content {
          display: grid;
          grid-template-columns: 300px 1fr 320px;
          gap: 1.5rem;
          min-height: calc(100vh - 250px);
        }
        .lobby-sidebar {
          display: flex;
          flex-direction: column;
        }
        .lobby-center {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }
        .ready-section {
          background: rgba(10, 10, 15, 0.8);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          padding: 1.5rem;
        }
        .start-section {
          background: rgba(10, 10, 15, 0.8);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          padding: 1.5rem;
        }
        .waiting-section {
          padding: 1rem 1.5rem;
          text-align: center;
          color: var(--color-text-muted, #666);
          background: rgba(10, 10, 15, 0.6);
          border: 1px solid rgba(255, 255, 255, 0.05);
          border-radius: var(--border-radius, 8px);
        }
        .waiting-section p {
          margin: 0;
        }
        .lobby-chat {
          display: flex;
          flex-direction: column;
          min-height: 400px;
        }
        @media (max-width: 1024px) {
          .lobby-content {
            grid-template-columns: 1fr;
          }
          .lobby-chat {
            min-height: 300px;
          }
        }
      `}</style>
    </div>
  );
}