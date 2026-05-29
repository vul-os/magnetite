import { useState, useRef, useEffect } from 'react';

export default function GameHUD({
  players,
  minimapData,
  chatMessages,
  onSendChat,
}) {
  const [showChat, setShowChat] = useState(false);
  const [chatInput, setChatInput] = useState('');
  const [showPlayerList, setShowPlayerList] = useState(true);
  const chatEndRef = useRef(null);

  useEffect(() => {
    if (chatEndRef.current) {
      chatEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
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
      <div className="hud-minimap">
        <div className="minimap-header">
          <span>Map</span>
        </div>
        <div className="minimap-content">
          <div className="minimap-grid">
            {minimapData.players?.map(player => (
              <div
                key={player.id}
                className="minimap-dot"
                style={{
                  left: `${player.position?.x || 50}%`,
                  top: `${player.position?.y || 50}%`,
                }}
                title={player.username}
              />
            ))}
            {minimapData.objectives?.map(obj => (
              <div
                key={obj.id}
                className="minimap-objective"
                style={{
                  left: `${obj.position?.x || 50}%`,
                  top: `${obj.position?.y || 50}%`,
                }}
                title={obj.name}
              />
            ))}
          </div>
          <div className="minimap-center" />
        </div>
      </div>

      {showPlayerList && (
        <div className="hud-player-list">
          <div className="player-list-header">
            <span>Players</span>
            <button className="toggle-btn" onClick={() => setShowPlayerList(false)}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M2 4l4 4 4-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
          </div>
          <div className="player-list-content">
            {sortedPlayers.map((player, index) => (
              <div key={player.id} className="player-row">
                <span className="player-rank">#{index + 1}</span>
                <span className="player-name">{player.username}</span>
                <span className="player-score">{player.score}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {!showPlayerList && (
        <button className="player-list-toggle" onClick={() => setShowPlayerList(true)}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M8 4v8M4 8h8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          Players
        </button>
      )}

      <div className={`hud-chat ${showChat ? 'expanded' : ''}`}>
        <button className="chat-toggle" onClick={() => setShowChat(!showChat)}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M14 2H2a1 1 0 00-1 1v8a1 1 0 001 1h3l3 3 3-3h3a1 1 0 001-1V3a1 1 0 00-1-1z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
          </svg>
          {chatMessages.length > 0 && <span className="chat-badge">{chatMessages.length}</span>}
        </button>

        {showChat && (
          <div className="chat-panel">
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
                placeholder="Press Enter to chat..."
                className="chat-input"
              />
            </form>
          </div>
        )}
      </div>

      <style>{`
        .hud-minimap {
          position: absolute;
          top: 80px;
          left: 1rem;
          width: 160px;
          background: rgba(10, 10, 15, 0.9);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .minimap-header {
          padding: 0.5rem 0.75rem;
          font-size: 0.625rem;
          text-transform: uppercase;
          letter-spacing: 0.1em;
          color: var(--color-text-muted, #888);
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }
        .minimap-content {
          position: relative;
          padding: 0.5rem;
        }
        .minimap-grid {
          position: relative;
          width: 100%;
          aspect-ratio: 1;
          background: linear-gradient(135deg, rgba(139, 92, 246, 0.1) 0%, rgba(59, 130, 246, 0.1) 100%);
          border-radius: 4px;
        }
        .minimap-dot {
          position: absolute;
          width: 8px;
          height: 8px;
          background: var(--color-accent, #8b5cf6);
          border-radius: 50%;
          transform: translate(-50%, -50%);
          box-shadow: 0 0 6px rgba(139, 92, 246, 0.6);
        }
        .minimap-objective {
          position: absolute;
          width: 12px;
          height: 12px;
          background: #fbbf24;
          border-radius: 2px;
          transform: translate(-50%, -50%);
          box-shadow: 0 0 6px rgba(251, 191, 36, 0.6);
        }
        .minimap-center {
          position: absolute;
          top: 50%;
          left: 50%;
          width: 4px;
          height: 4px;
          background: #fff;
          border-radius: 50%;
          transform: translate(-50%, -50%);
          opacity: 0.5;
        }
        .hud-player-list {
          position: absolute;
          top: 260px;
          left: 1rem;
          width: 200px;
          background: rgba(10, 10, 15, 0.9);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .player-list-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0.75rem;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--color-text-secondary, #888);
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }
        .toggle-btn {
          background: none;
          border: none;
          color: var(--color-text-muted, #666);
          cursor: pointer;
          padding: 0.25rem;
          display: flex;
          align-items: center;
          justify-content: center;
        }
        .toggle-btn:hover {
          color: #fff;
        }
        .player-list-content {
          max-height: 200px;
          overflow-y: auto;
        }
        .player-row {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.75rem;
          font-size: 0.8125rem;
        }
        .player-rank {
          font-size: 0.6875rem;
          color: var(--color-text-muted, #888);
          min-width: 24px;
        }
        .player-name {
          flex: 1;
          color: #fff;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .player-score {
          font-weight: 600;
          color: var(--color-accent, #8b5cf6);
          font-variant-numeric: tabular-nums;
        }
        .player-list-toggle {
          position: absolute;
          top: 260px;
          left: 1rem;
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: rgba(10, 10, 15, 0.9);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          color: #fff;
          font-size: 0.75rem;
          cursor: pointer;
        }
        .hud-chat {
          position: absolute;
          bottom: 1rem;
          left: 1rem;
        }
        .chat-toggle {
          position: relative;
          display: flex;
          align-items: center;
          justify-content: center;
          width: 44px;
          height: 44px;
          background: rgba(10, 10, 15, 0.9);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: 50%;
          color: #fff;
          cursor: pointer;
          transition: all 0.2s;
        }
        .chat-toggle:hover {
          background: rgba(139, 92, 246, 0.3);
          border-color: var(--color-accent, #8b5cf6);
        }
        .chat-badge {
          position: absolute;
          top: -4px;
          right: -4px;
          min-width: 18px;
          height: 18px;
          padding: 0 4px;
          background: var(--color-accent, #8b5cf6);
          border-radius: 9px;
          font-size: 0.625rem;
          font-weight: 600;
          color: #fff;
          display: flex;
          align-items: center;
          justify-content: center;
        }
        .chat-panel {
          position: absolute;
          bottom: 60px;
          left: 0;
          width: 300px;
          background: rgba(10, 10, 15, 0.95);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .chat-messages {
          max-height: 200px;
          overflow-y: auto;
          padding: 0.75rem;
        }
        .chat-message {
          font-size: 0.8125rem;
          line-height: 1.5;
          margin-bottom: 0.375rem;
        }
        .chat-sender {
          font-weight: 600;
          color: var(--color-accent, #8b5cf6);
        }
        .chat-content {
          color: #fff;
        }
        .chat-input-form {
          padding: 0.75rem;
          border-top: 1px solid rgba(255, 255, 255, 0.05);
        }
        .chat-input {
          width: 100%;
          padding: 0.625rem 0.875rem;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 4px);
          color: #fff;
          font-size: 0.8125rem;
        }
        .chat-input:focus {
          outline: none;
          border-color: var(--color-accent, #8b5cf6);
        }
        .chat-input::placeholder {
          color: var(--color-text-muted, #666);
        }
      `}</style>
    </>
  );
}
