import { useState, useRef, useEffect } from 'react';
import './GameHUD.css';

export default function GameHUD({
  players,
  minimapData,
  chatMessages,
  onSendChat,
}) {
  const [showChat, setShowChat]             = useState(false);
  const [chatInput, setChatInput]           = useState('');
  const [showPlayerList, setShowPlayerList] = useState(true);
  const chatEndRef                          = useRef(null);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chatMessages]);

  const handleSendChat = (e) => {
    e.preventDefault();
    if (!chatInput.trim()) return;
    onSendChat(chatInput);
    setChatInput('');
  };

  const sortedPlayers = [...players].sort((a, b) => b.score - a.score);

  return (
    <>
      {/* Minimap */}
      <div className="hud-minimap" role="region" aria-label="Minimap">
        <div className="minimap-header">// Map</div>
        <div className="minimap-content">
          <div className="minimap-grid" aria-hidden="true">
            {minimapData.players?.map(p => (
              <div
                key={p.id}
                className="minimap-dot"
                style={{ left: `${p.position?.x || 50}%`, top: `${p.position?.y || 50}%` }}
                title={p.username}
              />
            ))}
            {minimapData.objectives?.map(obj => (
              <div
                key={obj.id}
                className="minimap-objective"
                style={{ left: `${obj.position?.x || 50}%`, top: `${obj.position?.y || 50}%` }}
                title={obj.name}
              />
            ))}
          </div>
          <div className="minimap-center" aria-hidden="true" />
        </div>
      </div>

      {/* Player list */}
      {showPlayerList ? (
        <div className="hud-player-list" role="region" aria-label="Player rankings">
          <div className="player-list-header">
            <span>// Players</span>
            <button
              className="toggle-btn"
              onClick={() => setShowPlayerList(false)}
              aria-label="Hide player list"
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                <path d="M2 4l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          </div>
          <div className="player-list-content">
            {sortedPlayers.map((player, idx) => (
              <div key={player.id} className="player-row">
                <span className="player-rank">#{idx + 1}</span>
                <span className="player-name">{player.username}</span>
                <span className="player-score">{player.score.toLocaleString()}</span>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <button
          className="player-list-toggle"
          onClick={() => setShowPlayerList(true)}
          aria-label="Show player list"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M8 4v8M4 8h8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          Players
        </button>
      )}

      {/* Chat */}
      <div className={`hud-chat ${showChat ? 'expanded' : ''}`}>
        <button
          className="chat-toggle"
          onClick={() => setShowChat(v => !v)}
          aria-label={showChat ? 'Close chat' : 'Open chat'}
          aria-expanded={showChat}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M14 2H2a1 1 0 00-1 1v8a1 1 0 001 1h3l3 3 3-3h3a1 1 0 001-1V3a1 1 0 00-1-1z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
          </svg>
          {chatMessages.length > 0 && (
            <span className="chat-badge" aria-label={`${chatMessages.length} messages`}>
              {chatMessages.length}
            </span>
          )}
        </button>

        {showChat && (
          <div className="chat-panel" role="log" aria-live="polite" aria-label="Game chat">
            <div className="chat-messages">
              {chatMessages.map(msg => (
                <div key={msg.id} className="chat-message">
                  <span className="chat-sender">{msg.player}: </span>
                  <span className="chat-content">{msg.message}</span>
                </div>
              ))}
              <div ref={chatEndRef} />
            </div>
            <form className="chat-input-form" onSubmit={handleSendChat}>
              <input
                type="text"
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                placeholder="Press Enter to chat…"
                className="chat-input"
                aria-label="Chat message"
              />
            </form>
          </div>
        )}
      </div>
    </>
  );
}
