import { useEffect, useRef } from 'react';
import './MessageList.css';

/**
 * MessageList — renders chat messages grouped by author+time proximity.
 * Scrolls to bottom on new messages.
 */

function formatTime(isoString) {
  const d = new Date(isoString);
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function formatDate(isoString) {
  const d = new Date(isoString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);

  if (d.toDateString() === today.toDateString()) return 'Today';
  if (d.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return d.toLocaleDateString([], { month: 'long', day: 'numeric', year: 'numeric' });
}

function isSameGroup(a, b) {
  if (!a || !b) return false;
  if (a.authorId !== b.authorId) return false;
  const diff = new Date(b.createdAt) - new Date(a.createdAt);
  return diff < 5 * 60 * 1000; // 5-minute window
}

function needsDivider(a, b) {
  if (!a || !b) return false;
  return new Date(a.createdAt).toDateString() !== new Date(b.createdAt).toDateString();
}

export default function MessageList({ messages, currentUserId }) {
  const bottomRef = useRef(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  if (!messages || messages.length === 0) {
    return (
      <div className="message-list message-list--empty" role="log" aria-live="polite" aria-label="Messages">
        <div className="message-list__empty-state">
          <div className="message-list__empty-icon" aria-hidden="true">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
            </svg>
          </div>
          <p className="kicker">// no messages yet</p>
          <p className="message-list__empty-text">Be the first to say something!</p>
        </div>
      </div>
    );
  }

  const items = [];
  messages.forEach((msg, i) => {
    const prev = messages[i - 1];

    // Date divider
    if (needsDivider(prev, msg) || i === 0) {
      items.push(
        <div key={`divider-${i}`} className="message-divider" role="separator">
          <span className="message-divider__label">{formatDate(msg.createdAt)}</span>
        </div>
      );
    }

    const grouped = isSameGroup(prev, msg) && !needsDivider(prev, msg);
    const isOwn = msg.authorId === currentUserId;

    items.push(
      <article
        key={msg.id}
        className={`message-row${grouped ? ' message-row--grouped' : ''}${isOwn ? ' message-row--own' : ''}`}
        aria-label={`${msg.author ?? 'Unknown'}: ${msg.content}`}
      >
        {!grouped && (
          <div className="message-avatar" aria-hidden="true">
            {msg.avatarUrl ? (
              <img src={msg.avatarUrl} alt={msg.author} className="message-avatar__img" />
            ) : (
              <span className="message-avatar__initial">
                {(msg.author ?? 'U').charAt(0).toUpperCase()}
              </span>
            )}
          </div>
        )}

        {grouped && <div className="message-avatar-spacer" aria-hidden="true" />}

        <div className="message-body">
          {!grouped && (
            <div className="message-header">
              <span className="message-author">{msg.author}</span>
              {msg.role && <span className="message-role-badge">{msg.role}</span>}
              <time className="message-time" dateTime={msg.createdAt}>
                {formatTime(msg.createdAt)}
              </time>
            </div>
          )}

          <div className="message-content">
            {msg.content}
          </div>

          {/* Reactions */}
          {msg.reactions && msg.reactions.length > 0 && (
            <div className="message-reactions" aria-label="Reactions">
              {msg.reactions.map((r, ri) => (
                <button
                  key={ri}
                  className={`reaction${r.userReacted ? ' reaction--active' : ''}`}
                  aria-label={`${r.emoji} — ${r.count} reaction${r.count !== 1 ? 's' : ''}`}
                  title={r.label}
                >
                  <span aria-hidden="true">{r.emoji}</span>
                  <span className="reaction-count">{r.count}</span>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Hover timestamp for grouped messages */}
        {grouped && (
          <time className="message-time-hover" dateTime={msg.createdAt} aria-hidden="true">
            {formatTime(msg.createdAt)}
          </time>
        )}
      </article>
    );
  });

  return (
    <div
      className="message-list"
      role="log"
      aria-live="polite"
      aria-label="Channel messages"
    >
      {items}
      <div ref={bottomRef} aria-hidden="true" />
    </div>
  );
}
