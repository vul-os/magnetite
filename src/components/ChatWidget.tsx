import { useState } from 'react';
import './ChatWidget.css';

export default function ChatWidget({ isAdminOnline = false }) {
  const [message, setMessage] = useState('');

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!message.trim()) return;
    console.log('Chat message sent:', message);
    setMessage('');
  };

  return (
    <div className="chat-widget">
      {isAdminOnline ? (
        <>
          <div className="chat-status online">
            <span className="status-dot"></span>
            <span>Support is online</span>
          </div>

          <div className="chat-messages">
            <div className="message received">
              <div className="message-content">
                Hi there! How can I help you today?
              </div>
              <span className="message-time">Just now</span>
            </div>
          </div>

          <form className="chat-form" onSubmit={handleSubmit}>
            <input
              type="text"
              placeholder="Type your message..."
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              className="chat-input"
            />
            <button type="submit" className="chat-send" aria-label="Send message">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="22" y1="2" x2="11" y2="13"/>
                <polygon points="22 2 15 22 11 13 2 9 22 2"/>
              </svg>
            </button>
          </form>
        </>
      ) : (
        <div className="chat-offline">
          <div className="offline-icon">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
            </svg>
          </div>
          <h4>Live Chat Unavailable</h4>
          <p>Our support team is currently offline. Please leave us a message or contact us via email.</p>
          <a href="mailto:support@magnetite.gg" className="offline-contact">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
              <polyline points="22,6 12,13 2,6"/>
            </svg>
            Email Support
          </a>
        </div>
      )}
    </div>
  );
}
