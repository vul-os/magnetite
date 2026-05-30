import { useState, useEffect, useRef, useCallback } from 'react';
import { useMessages } from '../hooks/useMessages';
import { usePresence } from '../hooks/usePresence';
import { useCommsSocket } from '../hooks/useCommsSocket';
import { api } from '../api/client';
import './Messages.css';

// ── Icons (inline SVG to avoid import churn) ─────────────────────────────
function ChatBubbleIcon(props) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function SendIcon(props) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <line x1="22" y1="2" x2="11" y2="13" />
      <polygon points="22 2 15 22 11 13 2 9 22 2" />
    </svg>
  );
}

function PlusIcon(props) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

// ── Mock DM threads (fallback) ────────────────────────────────────────────
const MOCK_THREADS = [
  {
    id: '1',
    user: { id: '1', username: 'SpeedDemon', display_name: 'SpeedDemon', avatar: 'https://picsum.photos/seed/user1/100/100' },
    preview: 'Hey! Want to run some matchmaking?',
    unread: 2,
    updated_at: new Date(Date.now() - 300_000).toISOString(),
  },
  {
    id: '2',
    user: { id: '2', username: 'CosmicKing', display_name: 'CosmicKing', avatar: 'https://picsum.photos/seed/user2/100/100' },
    preview: 'Nice run on the Rust Racer circuit!',
    unread: 0,
    updated_at: new Date(Date.now() - 3_600_000).toISOString(),
  },
  {
    id: '3',
    user: { id: '3', username: 'StreamerX', display_name: 'StreamerX', avatar: 'https://picsum.photos/seed/user3/100/100' },
    preview: 'Check out my latest stream',
    unread: 1,
    updated_at: new Date(Date.now() - 86_400_000).toISOString(),
  },
];

// ── Helpers ───────────────────────────────────────────────────────────────
function formatTime(iso) {
  const d = new Date(iso);
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function formatDate(iso) {
  const d = new Date(iso);
  const now = new Date();
  const diff = now - d;
  if (diff < 86_400_000) return 'Today';
  if (diff < 172_800_000) return 'Yesterday';
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function formatThreadTime(iso) {
  const d = new Date(iso);
  const now = new Date();
  const diff = now - d;
  if (diff < 60_000) return 'now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h`;
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function getPresenceLabel(status) {
  switch (status) {
    case 'online':  return 'Online';
    case 'idle':    return 'Idle';
    case 'dnd':     return 'Do Not Disturb';
    case 'offline':
    default:        return 'Offline';
  }
}

function getInitials(name) {
  return (name ?? 'U').charAt(0).toUpperCase();
}

// ── Presence dot ─────────────────────────────────────────────────────────
function PresenceDot({ status, className = '' }) {
  return (
    <span
      className={`presence-dot ${status ?? 'offline'} ${className}`}
      aria-label={getPresenceLabel(status)}
    />
  );
}

// ── Message bubble ────────────────────────────────────────────────────────
function DmMessage({ message, isMine }) {
  const displayName = message.author?.display_name ?? message.author?.username ?? 'Unknown';
  const initial = getInitials(displayName);

  return (
    <div className={`dm-message${isMine ? ' mine' : ''}${message.pending ? ' pending' : ''}${message.failed ? ' failed' : ''}`}>
      <div className="dm-msg-avatar" aria-hidden="true">
        {message.author?.avatar ? (
          <img src={message.author.avatar} alt="" style={{ width: 32, height: 32, borderRadius: 'var(--radius-xs)', objectFit: 'cover', display: 'block' }} />
        ) : (
          initial
        )}
      </div>
      <div className="dm-msg-body">
        <div className="dm-msg-header">
          <span className="dm-msg-author">{isMine ? 'You' : displayName}</span>
          <time className="dm-msg-time" dateTime={message.created_at}>
            {formatTime(message.created_at)}
          </time>
        </div>
        <div className="dm-msg-bubble">{message.content}</div>
        {message.failed && (
          <div className="dm-msg-failed-hint" role="alert">Failed — tap to retry</div>
        )}
      </div>
    </div>
  );
}

// ── Composer ──────────────────────────────────────────────────────────────
function DmComposer({ onSend, disabled, recipientName }) {
  const [value, setValue] = useState('');
  const textareaRef = useRef(null);

  const handleKeyDown = useCallback(
    (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (value.trim() && !disabled) {
          onSend(value.trim());
          setValue('');
        }
      }
    },
    [value, disabled, onSend]
  );

  const handleSend = useCallback(() => {
    if (value.trim() && !disabled) {
      onSend(value.trim());
      setValue('');
      textareaRef.current?.focus();
    }
  }, [value, disabled, onSend]);

  return (
    <div className="dm-composer">
      <div className="dm-composer-inner">
        <textarea
          ref={textareaRef}
          className="dm-composer-input"
          placeholder={`Message ${recipientName ?? ''}…`}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={1}
          disabled={disabled}
          aria-label={`Message ${recipientName ?? ''}`}
        />
        <button
          className="dm-send-btn"
          onClick={handleSend}
          disabled={!value.trim() || disabled}
          aria-label="Send message"
        >
          <SendIcon />
        </button>
      </div>
    </div>
  );
}

// ── Conversation view ─────────────────────────────────────────────────────
function DmConversation({ thread, isConnected }) {
  const { user } = thread;
  const { presenceMap } = usePresence([user.id]);
  const presence = presenceMap[user.id] ?? { status: 'offline' };

  const {
    messages,
    loading,
    hasMore,
    loadMore,
    postMessage,
    appendMessage,
  } = useMessages(null, { isDM: true, dmUserId: user.id });

  const handleIncomingDM = useCallback(
    (msg) => {
      if (msg.sender_id === user.id) {
        appendMessage({ ...msg, pending: false });
      }
    },
    [user.id, appendMessage]
  );

  const { typingUsers, sendDMMessage, sendTypingStop } = useCommsSocket({
    onDM: handleIncomingDM,
  });

  const bottomRef = useRef(null);
  const isTypingRef = useRef(false);
  const typingTimeoutRef = useRef(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSend = useCallback(
    async (content) => {
      // Stop typing indicator
      if (isTypingRef.current) {
        sendTypingStop();
        isTypingRef.current = false;
        clearTimeout(typingTimeoutRef.current);
      }
      // Send over WS for instant delivery
      sendDMMessage(content, user.id);
      // Also persist via REST
      await postMessage(content);
    },
    [sendDMMessage, sendTypingStop, postMessage, user.id]
  );

  // Group messages by date for dividers
  const groupedMessages = [];
  let lastDate = null;
  for (const msg of messages) {
    const dateLabel = formatDate(msg.created_at);
    if (dateLabel !== lastDate) {
      groupedMessages.push({ type: 'divider', label: dateLabel, key: `divider-${dateLabel}` });
      lastDate = dateLabel;
    }
    groupedMessages.push({ type: 'message', msg, key: msg.id });
  }

  const typingList = Object.values(typingUsers);

  return (
    <>
      {/* Header */}
      <div className="dm-conv-header">
        <div className="dm-conv-avatar">
          {user.avatar ? (
            <img src={user.avatar} alt={`${user.username} avatar`} className="dm-conv-avatar-img" />
          ) : (
            <div className="dm-conv-avatar-img" aria-hidden="true" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--font-mono)', fontWeight: 800, color: '#000', background: 'var(--gradient-primary)' }}>
              {getInitials(user.display_name ?? user.username)}
            </div>
          )}
          <PresenceDot status={presence.status} />
        </div>
        <div>
          <div className="dm-conv-name">{user.display_name ?? user.username}</div>
          <div className={`dm-conv-status ${presence.status}`}>
            {presence.activity ?? getPresenceLabel(presence.status)}
          </div>
        </div>
      </div>

      {/* Connection banner */}
      {!isConnected && (
        <div className="dm-conn-banner" role="status" aria-live="polite">
          Reconnecting to real-time…
        </div>
      )}

      {/* Messages */}
      <div className="dm-messages-area" role="log" aria-label="Direct messages" aria-live="polite">
        {hasMore && (
          <div className="dm-load-more">
            <button onClick={loadMore} disabled={loading}>
              {loading ? 'Loading…' : 'Load older messages'}
            </button>
          </div>
        )}
        {loading && messages.length === 0 && (
          <div className="dm-messages-loading" aria-label="Loading messages">
            <div className="dm-spinner" aria-hidden="true" />
            Loading messages…
          </div>
        )}
        {groupedMessages.map((item) =>
          item.type === 'divider' ? (
            <div key={item.key} className="dm-date-divider" role="separator">
              <span>{item.label}</span>
            </div>
          ) : (
            <DmMessage
              key={item.key}
              message={item.msg}
              isMine={item.msg.author?.id === 'me'}
            />
          )
        )}
        <div ref={bottomRef} />
      </div>

      {/* Typing indicator */}
      <div className="dm-typing" aria-live="polite" aria-atomic="true">
        {typingList.length > 0 && (
          <>
            <span className="dm-typing-dots" aria-hidden="true">
              <span /><span /><span />
            </span>
            {typingList.join(', ')} {typingList.length === 1 ? 'is' : 'are'} typing…
          </>
        )}
      </div>

      {/* Composer */}
      <DmComposer
        onSend={handleSend}
        disabled={false}
        recipientName={user.display_name ?? user.username}
      />
    </>
  );
}

// ── Thread item ───────────────────────────────────────────────────────────
function DmThreadItem({ thread, active, presence, onClick }) {
  const { user, preview, unread, updated_at } = thread;
  return (
    <button
      className={`dm-thread-item${active ? ' active' : ''}`}
      onClick={onClick}
      aria-current={active ? 'true' : undefined}
      aria-label={`Conversation with ${user.display_name ?? user.username}${unread > 0 ? `, ${unread} unread` : ''}`}
    >
      <div className="dm-thread-avatar">
        {user.avatar ? (
          <img src={user.avatar} alt="" className="dm-thread-avatar-img" />
        ) : (
          <div className="dm-thread-avatar-img" aria-hidden="true" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--font-mono)', fontWeight: 800, color: '#000', background: 'var(--gradient-primary)' }}>
            {getInitials(user.display_name ?? user.username)}
          </div>
        )}
        <PresenceDot status={presence?.status ?? 'offline'} />
      </div>
      <div className="dm-thread-meta">
        <div className="dm-thread-name">{user.display_name ?? user.username}</div>
        {preview && <div className="dm-thread-preview">{preview}</div>}
      </div>
      <div className="dm-thread-time">{formatThreadTime(updated_at)}</div>
      {unread > 0 && (
        <span className="dm-unread-badge" aria-label={`${unread} unread`}>{unread}</span>
      )}
    </button>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────
export default function Messages() {
  const [threads, setThreads] = useState(MOCK_THREADS);
  const [activeThreadId, setActiveThreadId] = useState(null);
  const [searchQuery, setSearchQuery] = useState('');

  // Presence for all users in threads
  const userIds = threads.map((t) => t.user.id);
  const { presenceMap, setPresence } = usePresence(userIds);

  // Socket for DM events and presence updates
  const { isConnected } = useCommsSocket({
    onPresence: useCallback(
      (msg) => {
        setPresence(msg.user_id, { status: msg.status, activity: msg.activity ?? null });
      },
      [setPresence]
    ),
    onDM: useCallback(
      (msg) => {
        // Update thread preview when a DM arrives
        setThreads((prev) =>
          prev.map((t) =>
            t.user.id === msg.sender_id
              ? { ...t, preview: msg.content, updated_at: new Date().toISOString(), unread: (t.unread ?? 0) + 1 }
              : t
          )
        );
      },
      []
    ),
  });

  // Fetch real DM threads if API is available
  useEffect(() => {
    let cancelled = false;
    api.messages?.listDMThreads?.()
      .then((data) => {
        if (!cancelled && Array.isArray(data) && data.length > 0) {
          setThreads(data);
        }
      })
      .catch(() => { /* use mock */ });
    return () => { cancelled = true; };
  }, []);

  const activeThread = threads.find((t) => t.id === activeThreadId) ?? null;

  // Clear unread count on open
  const handleSelectThread = useCallback((threadId) => {
    setActiveThreadId(threadId);
    setThreads((prev) =>
      prev.map((t) => (t.id === threadId ? { ...t, unread: 0 } : t))
    );
  }, []);

  const filteredThreads = searchQuery.trim()
    ? threads.filter((t) =>
        (t.user.display_name ?? t.user.username)
          .toLowerCase()
          .includes(searchQuery.toLowerCase())
      )
    : threads;

  return (
    <main className="messages-page" id="main-content" aria-label="Direct messages">
      {/* ── Thread sidebar ── */}
      <aside className="dm-sidebar" aria-label="Conversations">
        <div className="dm-sidebar-header">
          <h2>Direct Messages</h2>
          <button className="dm-new-btn" aria-label="New message">
            <PlusIcon />
          </button>
        </div>
        <div className="dm-search">
          <input
            type="search"
            placeholder="Search…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            aria-label="Search conversations"
          />
        </div>
        <div className="dm-thread-list" role="list">
          {filteredThreads.length === 0 ? (
            <div className="dm-empty-threads">
              <ChatBubbleIcon />
              <p>No conversations yet</p>
            </div>
          ) : (
            filteredThreads.map((thread) => (
              <div key={thread.id} role="listitem">
                <DmThreadItem
                  thread={thread}
                  active={thread.id === activeThreadId}
                  presence={presenceMap[thread.user.id]}
                  onClick={() => handleSelectThread(thread.id)}
                />
              </div>
            ))
          )}
        </div>
      </aside>

      {/* ── Conversation area ── */}
      <section className="dm-conversation" aria-label="Conversation">
        {activeThread ? (
          <DmConversation
            key={activeThread.id}
            thread={activeThread}
            isConnected={isConnected}
          />
        ) : (
          <div className="dm-no-conversation">
            <div className="dm-no-conversation-icon" aria-hidden="true">
              <ChatBubbleIcon />
            </div>
            <h3>Direct Messages</h3>
            <p>Select a conversation to start chatting</p>
          </div>
        )}
      </section>
    </main>
  );
}
