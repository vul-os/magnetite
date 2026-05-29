import { useState, useRef, useEffect } from 'react';
import Button from './common/Button';

export default function LobbyChat({
  messages = [],
  currentUserId,
  onSendMessage,
  disabled = false,
}) {
  const [input, setInput] = useState('');
  const messagesEndRef = useRef(null);
  const inputRef = useRef(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!input.trim() || disabled) return;
    onSendMessage(input.trim());
    setInput('');
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  const formatTime = (timestamp) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  };

  return (
    <div className="lobby-chat">
      <div className="chat-header">
        <h3>Chat</h3>
      </div>
      <div className="chat-messages">
        {messages.length === 0 ? (
          <div className="chat-empty">
            <p>No messages yet. Start the conversation!</p>
          </div>
        ) : (
          messages.map((msg) => {
            const isOwnMessage = msg.senderId === currentUserId;
            return (
              <div
                key={msg.id}
                className={`chat-message ${isOwnMessage ? 'own' : ''}`}
              >
                <div className="message-meta">
                  <span className="message-sender">{msg.senderName}</span>
                  <span className="message-time">{formatTime(msg.timestamp)}</span>
                </div>
                <div className="message-content">{msg.content}</div>
              </div>
            );
          })
        )}
        <div ref={messagesEndRef} />
      </div>
      <form className="chat-input-form" onSubmit={handleSubmit}>
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={disabled ? 'Chat disabled' : 'Type a message...'}
          disabled={disabled}
          className="chat-input"
        />
        <Button
          type="submit"
          variant="primary"
          size="sm"
          disabled={disabled || !input.trim()}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M14 2L1 7l5 2m8-7l-6 5 6 5-1-3m5-2H7m3-3H4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </Button>
      </form>

      <style>{`
        .lobby-chat {
          display: flex;
          flex-direction: column;
          height: 100%;
          background: rgba(10, 10, 15, 0.8);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 8px);
          overflow: hidden;
        }
        .chat-header {
          padding: 1rem;
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }
        .chat-header h3 {
          margin: 0;
          font-size: 0.875rem;
          font-weight: 600;
          color: var(--color-text-primary, #fff);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 1rem;
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }
        .chat-empty {
          flex: 1;
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-muted, #666);
          font-size: 0.875rem;
        }
        .chat-empty p {
          margin: 0;
        }
        .chat-message {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
          max-width: 85%;
        }
        .chat-message.own {
          align-self: flex-end;
        }
        .message-meta {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0 0.25rem;
        }
        .message-sender {
          font-size: 0.75rem;
          font-weight: 600;
          color: var(--color-accent, #8b5cf6);
        }
        .chat-message.own .message-sender {
          color: var(--color-success, #22c55e);
        }
        .message-time {
          font-size: 0.625rem;
          color: var(--color-text-muted, #666);
        }
        .message-content {
          padding: 0.625rem 0.875rem;
          background: rgba(255, 255, 255, 0.05);
          border-radius: 12px;
          border-top-left-radius: 4px;
          font-size: 0.875rem;
          color: var(--color-text-primary, #fff);
          line-height: 1.4;
          word-wrap: break-word;
        }
        .chat-message.own .message-content {
          background: rgba(139, 92, 246, 0.2);
          border-top-left-radius: 12px;
          border-top-right-radius: 4px;
        }
        .chat-input-form {
          display: flex;
          gap: 0.5rem;
          padding: 1rem;
          border-top: 1px solid rgba(255, 255, 255, 0.05);
        }
        .chat-input {
          flex: 1;
          padding: 0.625rem 0.875rem;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: var(--border-radius, 4px);
          color: var(--color-text-primary, #fff);
          font-size: 0.875rem;
        }
        .chat-input:focus {
          outline: none;
          border-color: var(--color-accent, #8b5cf6);
        }
        .chat-input::placeholder {
          color: var(--color-text-muted, #666);
        }
        .chat-input:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}