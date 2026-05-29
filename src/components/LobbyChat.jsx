import { useState, useRef, useEffect } from 'react';
import Button from './common/Button';
import './LobbyChat.css';

export default function LobbyChat({
  messages = [],
  currentUserId,
  onSendMessage,
  disabled = false,
}) {
  const [input, setInput]     = useState('');
  const messagesEndRef        = useRef(null);
  const inputRef              = useRef(null);

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

  const formatTime = (ts) =>
    new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  return (
    <div className="lobby-chat" role="region" aria-label="Lobby chat">
      <div className="chat-header">
        <h3>// Chat</h3>
      </div>

      <div className="chat-messages" role="log" aria-live="polite" aria-label="Chat messages">
        {messages.length === 0 ? (
          <div className="chat-empty">
            <p>No messages yet. Start the conversation!</p>
          </div>
        ) : (
          messages.map((msg) => {
            const isOwn = msg.senderId === currentUserId;
            return (
              <div key={msg.id} className={`chat-message ${isOwn ? 'own' : ''}`}>
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
          placeholder={disabled ? 'Chat disabled' : 'Type a message…'}
          disabled={disabled}
          className="chat-input"
          aria-label="Chat message"
        />
        <Button
          type="submit"
          variant="primary"
          size="sm"
          isDisabled={disabled || !input.trim()}
          aria-label="Send message"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M14 2L1 7l5 2m8-7l-6 5 6 5-1-3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </Button>
      </form>
    </div>
  );
}
