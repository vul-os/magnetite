import { useState, useRef, useCallback } from 'react';
import './MessageComposer.css';

/**
 * MessageComposer — text input with emoji, attachment, and send controls.
 * onSend(text) is called with the message string.
 */
export default function MessageComposer({ channel, onSend, disabled = false }) {
  const [value, setValue] = useState('');
  const [typing, setTyping]   = useState(false);
  const textareaRef = useRef(null);
  const typingTimer = useRef(null);

  const handleChange = useCallback((e) => {
    setValue(e.target.value);

    // Simulate "typing" indicator state
    setTyping(true);
    clearTimeout(typingTimer.current);
    typingTimer.current = setTimeout(() => setTyping(false), 2000);
  }, []);

  const handleSubmit = useCallback((e) => {
    e?.preventDefault();
    const text = value.trim();
    if (!text || disabled) return;
    onSend?.(text);
    setValue('');
    setTyping(false);
    textareaRef.current?.focus();
  }, [value, disabled, onSend]);

  const handleKeyDown = useCallback((e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }, [handleSubmit]);

  const placeholder = channel
    ? `Message #${channel.name}`
    : 'Send a message…';

  return (
    <form
      className="message-composer"
      onSubmit={handleSubmit}
      role="form"
      aria-label={`Send message to ${channel?.name ?? 'channel'}`}
    >
      <div className="composer-inner">
        {/* Attach button */}
        <button
          type="button"
          className="composer-action-btn"
          aria-label="Attach file"
          title="Attach file"
          disabled={disabled}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
          </svg>
        </button>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          className="composer-input"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          aria-label={placeholder}
          rows={1}
          disabled={disabled}
          autoComplete="off"
          spellCheck="true"
        />

        {/* Right controls */}
        <div className="composer-right">
          {/* Emoji */}
          <button
            type="button"
            className="composer-action-btn"
            aria-label="Add emoji"
            title="Emoji"
            disabled={disabled}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="10"/>
              <path d="M8 13s1.5 2 4 2 4-2 4-2"/>
              <line x1="9" y1="9" x2="9.01" y2="9"/>
              <line x1="15" y1="9" x2="15.01" y2="9"/>
            </svg>
          </button>

          {/* GIF */}
          <button
            type="button"
            className="composer-action-btn composer-action-btn--label"
            aria-label="Send GIF"
            title="GIF"
            disabled={disabled}
          >
            GIF
          </button>

          {/* Send */}
          <button
            type="submit"
            className={`composer-send-btn${value.trim() ? ' composer-send-btn--active' : ''}`}
            aria-label="Send message"
            disabled={!value.trim() || disabled}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/>
            </svg>
          </button>
        </div>
      </div>

      {/* Typing indicator (local demo — in Wave 7 wired to WS) */}
      <div className="composer-footer" aria-live="polite" aria-atomic="true">
        {typing && (
          <span className="typing-indicator" aria-label="You are typing">
            <span className="typing-dot" aria-hidden="true" />
            <span className="typing-dot" aria-hidden="true" />
            <span className="typing-dot" aria-hidden="true" />
            <span className="typing-text">You are typing…</span>
          </span>
        )}
        <span className="composer-hint">
          <kbd>Shift</kbd>+<kbd>Enter</kbd> for new line
        </span>
      </div>
    </form>
  );
}
