/**
 * GameOverlay — collapsible in-game chat + voice panel.
 *
 * Usage:
 *   <GameOverlay channelId="ch-123" voiceRoomId="vr-1" label="Match Lobby" />
 *
 * Props:
 *   channelId    {string|null}  — text channel to bind. Null → mock fallback.
 *   voiceRoomId  {string|null}  — voice room to auto-join. Null → voice tab shows empty state.
 *   label        {string}       — human label shown in the header (e.g. "Match Lobby").
 *   defaultOpen  {boolean}      — start expanded (default false).
 *   comms        {object|null}  — pass the result of useComms() from the parent if available,
 *                                 or omit to fall back to mock/demo data.
 *
 * Hotkey: Tab or backtick (`) toggles the panel open/closed (unless focus is in an input).
 *
 * Because useComms() throws when no CommsProvider is mounted, pages that DO have
 * CommsProvider (Playground, GameLobby, Spectator) should call useComms() themselves
 * and pass the result as the `comms` prop. Pages that do not have CommsProvider simply
 * omit the prop and the overlay renders in demo mode with mock data.
 */

import {
  useState,
  useEffect,
  useRef,
  useCallback,
} from 'react';
import './GameOverlay.css';

// ── Mock fallback data ────────────────────────────────────────────────────────
const MOCK_MESSAGES = [
  {
    id: 'm1',
    author: { id: 'sys', username: 'System', display_name: 'System' },
    content: 'Match started — good luck!',
    created_at: new Date(Date.now() - 120_000).toISOString(),
  },
  {
    id: 'm2',
    author: { id: 'p1', username: 'player_one', display_name: 'Player One' },
    content: "gg everyone, let's go!",
    created_at: new Date(Date.now() - 60_000).toISOString(),
  },
];

const MOCK_PARTICIPANTS = [
  { id: 'p1', username: 'player_one',   display_name: 'Player One',   muted: false, deafened: false, speaking: true  },
  { id: 'p2', username: 'player_two',   display_name: 'Player Two',   muted: true,  deafened: false, speaking: false },
  { id: 'p3', username: 'player_three', display_name: 'Player Three', muted: false, deafened: true,  speaking: false },
];

// ── Helpers ───────────────────────────────────────────────────────────────────
function formatTs(isoString) {
  try {
    return new Date(isoString).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    });
  } catch {
    return '';
  }
}

function initials(name = '') {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('');
}

// ── Inline SVG icons ──────────────────────────────────────────────────────────
function IconChat() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M2 2h12a1 1 0 011 1v8a1 1 0 01-1 1H5l-3 2V3a1 1 0 011-1z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function IconMic({ off }) {
  if (off) {
    return (
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M2 2l12 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <path d="M6 6v2a2 2 0 003.46 1.38" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <path d="M4 8a4 4 0 007.32 2.24M8 14v-2M5 14h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="5" y="1" width="6" height="9" rx="3" stroke="currentColor" strokeWidth="1.5" />
      <path d="M3 8a5 5 0 0010 0M8 14v-2M5 14h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function IconDeafen({ off }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {off ? (
        <>
          <path d="M2 2l12 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <path d="M4 8a4 4 0 007.95-.9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <path d="M3 8c0-.7.12-1.36.34-1.98" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </>
      ) : (
        <path
          d="M2 8a6 6 0 1112 0v3a1 1 0 01-1 1h-1a1 1 0 01-1-1v-2a1 1 0 011-1 4 4 0 00-8 0 1 1 0 011 1v2a1 1 0 01-1 1H3a1 1 0 01-1-1V8z"
          stroke="currentColor"
          strokeWidth="1.5"
        />
      )}
    </svg>
  );
}

function IconLeave() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M6 14H3a1 1 0 01-1-1V3a1 1 0 011-1h3M10 11l3-3-3-3M6 8h7"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function IconSend() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M14 2L2 7l5 2 2 5 5-12z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    </svg>
  );
}

function IconClose() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function IconVoice() {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M3 6a5 5 0 0010 0" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <path d="M8 1v10M5 14h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function SpeakingDots() {
  return (
    <span className="overlay-speaking-dots" aria-hidden="true">
      <span />
      <span />
      <span />
    </span>
  );
}

function MuteBadge() {
  return (
    <span className="overlay-participant-mute-badge" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none">
        <path d="M2 2l12 12M8 1v8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </span>
  );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CHAT TAB
// ═══════════════════════════════════════════════════════════════════════════════
function ChatTab({ channelId, comms }) {
  const listRef = useRef(null);
  const [input, setInput] = useState('');

  const isMock = comms == null;

  const messages        = isMock ? MOCK_MESSAGES     : (comms.messages ?? MOCK_MESSAGES);
  const typingUsers     = isMock ? {}                : (comms.typingUsers ?? {});
  const isConnected     = isMock ? false             : (comms.isConnected ?? false);
  const sendChatMsg     = isMock ? null              : comms.sendChatMessage;
  const postMessage     = isMock ? null              : comms.postMessage;
  const sendTypingStart = isMock ? null              : comms.sendTypingStart;
  const sendTypingStop  = isMock ? null              : comms.sendTypingStop;

  const typingList = Object.values(typingUsers);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages.length]);

  const handleSubmit = useCallback(
    (e) => {
      e.preventDefault();
      const trimmed = input.trim();
      if (!trimmed) return;
      setInput('');
      if (sendTypingStop) sendTypingStop();
      if (postMessage) {
        postMessage(trimmed);
      } else if (sendChatMsg) {
        sendChatMsg(trimmed, channelId);
      }
    },
    [input, channelId, postMessage, sendChatMsg, sendTypingStop]
  );

  const handleInputChange = useCallback(
    (e) => {
      setInput(e.target.value);
      if (sendTypingStart) sendTypingStart();
    },
    [sendTypingStart]
  );

  const handleInputKeyDown = useCallback(
    (e) => {
      // Swallow Tab inside input so it does not toggle the overlay panel
      e.stopPropagation();
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit(e);
      }
    },
    [handleSubmit]
  );

  return (
    <div className="overlay-tab-panel" role="tabpanel" aria-label="Chat">
      <div
        className="overlay-status-bar"
        data-connected={String(isConnected)}
        aria-live="polite"
        aria-label={
          isConnected ? 'Connected to chat' : isMock ? 'Chat — demo mode' : 'Chat disconnected'
        }
      >
        <span className="overlay-status-dot" aria-hidden="true" />
        {isConnected ? 'Live' : isMock ? 'Demo' : 'Offline'}
      </div>

      <div
        className="overlay-chat-messages"
        ref={listRef}
        role="log"
        aria-label="Chat messages"
        aria-live="polite"
        aria-relevant="additions"
      >
        {messages.length === 0 ? (
          <div className="overlay-chat-empty">
            <IconChat />
            <span>No messages yet</span>
          </div>
        ) : (
          messages.map((msg) => (
            <div
              key={msg.id}
              className="overlay-chat-msg"
              data-pending={String(!!msg.pending)}
              data-failed={String(!!msg.failed)}
            >
              <div className="overlay-chat-msg-header">
                <span className="overlay-chat-author">
                  {msg.author?.display_name ?? msg.author?.username ?? 'Unknown'}
                </span>
                <span className="overlay-chat-ts">{formatTs(msg.created_at)}</span>
              </div>
              <span className="overlay-chat-text">{msg.content}</span>
            </div>
          ))
        )}

        {typingList.length > 0 && (
          <div className="overlay-typing-indicator" aria-live="polite" aria-atomic="true">
            <span className="overlay-typing-dots" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
            <span>
              {typingList.slice(0, 2).join(', ')}{' '}
              {typingList.length === 1 ? 'is' : 'are'} typing…
            </span>
          </div>
        )}
      </div>

      <form className="overlay-chat-form" onSubmit={handleSubmit} aria-label="Send chat message">
        <label htmlFor="overlay-chat-input" className="sr-only">
          Message
        </label>
        <input
          id="overlay-chat-input"
          type="text"
          className="overlay-chat-input"
          placeholder={channelId ? 'Message…' : 'No channel — demo mode'}
          value={input}
          onChange={handleInputChange}
          onKeyDown={handleInputKeyDown}
          autoComplete="off"
          maxLength={500}
        />
        <button
          type="submit"
          className="overlay-chat-send-btn"
          disabled={!input.trim()}
          aria-label="Send message"
        >
          <IconSend />
        </button>
      </form>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════════
// VOICE TAB
// ═══════════════════════════════════════════════════════════════════════════════
function VoiceTab({ voiceRoomId, comms }) {
  const isMock = comms == null;

  const currentRoom    = isMock ? { id: 'mock-room', name: 'General Voice' } : (comms.currentRoom ?? null);
  const muted          = isMock ? false  : (comms.muted ?? false);
  const deafened       = isMock ? false  : (comms.deafened ?? false);
  const participants   = isMock ? MOCK_PARTICIPANTS  : (comms.voiceParticipants ?? []);
  const toggleMute     = isMock ? null   : comms.toggleMute;
  const toggleDeafen   = isMock ? null   : comms.toggleDeafen;
  const joinVoiceRoom  = isMock ? null   : comms.joinVoiceRoom;
  const leaveVoiceRoom = isMock ? null   : comms.leaveVoiceRoom;
  const voiceConnected = isMock ? false  : (comms.voiceConnected ?? false);
  const currentRoomId  = currentRoom?.id ?? null;

  // Auto-join the target room when voiceRoomId is provided and we are not in it
  useEffect(() => {
    if (!voiceRoomId || !joinVoiceRoom) return;
    if (currentRoomId === voiceRoomId) return;
    joinVoiceRoom(voiceRoomId).catch(() => {
      // Swallow — no voice server available yet
    });
  }, [voiceRoomId, currentRoomId, joinVoiceRoom]);

  if (!currentRoom) {
    return (
      <div className="overlay-voice-panel" role="tabpanel" aria-label="Voice">
        <div className="overlay-voice-empty">
          <svg
            className="overlay-voice-empty-icon"
            viewBox="0 0 16 16"
            fill="none"
            aria-hidden="true"
          >
            <path d="M3 6a5 5 0 0010 0" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <path d="M8 1v10M5 14h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
          <p className="overlay-voice-empty-title">Not in a voice room</p>
          <p className="overlay-voice-empty-hint">
            {voiceRoomId ? 'Joining voice room…' : 'No voice room assigned to this session.'}
          </p>
        </div>
      </div>
    );
  }

  const anySpeaking = participants.some((p) => p.speaking);

  return (
    <div className="overlay-voice-panel" role="tabpanel" aria-label="Voice">
      <div className="overlay-voice-room-info">
        <span className="overlay-voice-room-name">{currentRoom.name ?? 'Voice Room'}</span>
        <span className="overlay-voice-room-meta" aria-live="polite">
          {voiceConnected ? `${participants.length} connected` : 'connecting…'}
        </span>
      </div>

      <div className="overlay-voice-controls" role="group" aria-label="Voice controls">
        <button
          type="button"
          className="overlay-voice-ctrl-btn"
          aria-pressed={muted}
          aria-label={muted ? 'Unmute microphone' : 'Mute microphone'}
          onClick={toggleMute ?? undefined}
          disabled={!toggleMute}
        >
          <IconMic off={muted} />
          {muted ? 'Muted' : 'Mic'}
        </button>

        <button
          type="button"
          className="overlay-voice-ctrl-btn"
          aria-pressed={deafened}
          aria-label={deafened ? 'Undeafen' : 'Deafen'}
          onClick={toggleDeafen ?? undefined}
          disabled={!toggleDeafen}
        >
          <IconDeafen off={deafened} />
          {deafened ? 'Deafened' : 'Sound'}
        </button>

        {leaveVoiceRoom && (
          <button
            type="button"
            className="overlay-voice-leave-btn"
            aria-label="Leave voice room"
            onClick={leaveVoiceRoom}
          >
            <IconLeave />
            Leave
          </button>
        )}
      </div>

      <ul
        className="overlay-voice-participants"
        aria-label={`Voice participants — ${participants.length} in room${anySpeaking ? ', someone is speaking' : ''}`}
      >
        {participants.length === 0 && (
          <li
            style={{
              padding: '0.875rem',
              color: 'var(--color-text-muted)',
              fontFamily: 'var(--font-mono)',
              fontSize: '11px',
              textAlign: 'center',
            }}
          >
            No other participants
          </li>
        )}
        {participants.map((p) => {
          const isMuted    = p.muted || p.deafened;
          const isSpeaking = p.speaking && !isMuted;
          const rowLabel   = [
            p.display_name ?? p.username,
            isSpeaking && 'speaking',
            isMuted    && 'muted',
          ].filter(Boolean).join(', ');

          return (
            <li key={p.id} className="overlay-participant-row" aria-label={rowLabel}>
              <div
                className="overlay-participant-avatar"
                data-speaking={String(isSpeaking)}
                aria-hidden="true"
              >
                {initials(p.display_name ?? p.username)}
                {isMuted && <MuteBadge />}
              </div>

              <div className="overlay-participant-info">
                <span className="overlay-participant-name">
                  {p.display_name ?? p.username}
                </span>
                <span className="overlay-participant-state">
                  {isSpeaking && <>speaking<SpeakingDots /></>}
                  {!isSpeaking && isMuted  && 'muted'}
                  {!isSpeaking && !isMuted && 'listening'}
                </span>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN EXPORTED COMPONENT
// ═══════════════════════════════════════════════════════════════════════════════
/**
 * GameOverlay
 *
 * Pages that have a CommsProvider above them should call useComms() and pass the
 * result as the `comms` prop.  Pages without a provider omit `comms` and the
 * overlay renders in demo mode with mock data — no errors thrown.
 */
export default function GameOverlay({
  channelId   = null,
  voiceRoomId = null,
  label       = 'In-Game',
  defaultOpen = false,
  comms       = null,        // pass useComms() result from the parent page
}) {
  const [open, setOpen]         = useState(defaultOpen);
  const [activeTab, setActiveTab] = useState('chat');

  // Wire up the target channel in the context when channelId prop changes
  useEffect(() => {
    if (!comms?.selectChannel || !channelId) return;
    comms.selectChannel(channelId);
  }, [channelId, comms]);

  // ── Hotkey: Tab or backtick toggles overlay ──────────────────────────────
  useEffect(() => {
    function handler(e) {
      const tag = document.activeElement?.tagName?.toLowerCase();
      if (tag === 'input' || tag === 'textarea' || tag === 'select') return;
      if (e.key === 'Tab' || e.key === '`') {
        e.preventDefault();
        setOpen((o) => !o);
      }
    }
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const toggle = useCallback(() => setOpen((o) => !o), []);
  const close  = useCallback(() => setOpen(false), []);

  // Derived counts for badges and fab speaking indicator
  const msgCount = comms
    ? (comms.messages?.length ?? 0)
    : MOCK_MESSAGES.length;

  const participantCount = comms
    ? (comms.voiceParticipants?.length ?? 0)
    : MOCK_PARTICIPANTS.length;

  const anySpeaking = comms
    ? (comms.voiceParticipants?.some((p) => p.speaking) ?? false)
    : MOCK_PARTICIPANTS.some((p) => p.speaking);

  return (
    <>
      {/* ── FAB: always visible so players can open the overlay ─────────── */}
      <button
        type="button"
        className="game-overlay-fab"
        aria-expanded={open}
        aria-controls="game-overlay-panel"
        aria-label={open ? 'Close in-game overlay' : 'Open in-game overlay (Tab or `)'}
        onClick={toggle}
        title="Toggle in-game overlay (Tab / `)"
      >
        <span className="overlay-fab-icon" aria-hidden="true">
          <IconChat />
        </span>
        <span>{label}</span>
        {anySpeaking && (
          <span className="overlay-fab-speaking-dot" aria-label="Someone is speaking" />
        )}
      </button>

      {/* ── Panel ───────────────────────────────────────────────────────── */}
      {open && (
        <div
          id="game-overlay-panel"
          className="game-overlay-panel"
          role="dialog"
          aria-modal="false"
          aria-label={`${label} — in-game overlay`}
        >
          {/* Header: kicker + tab bar + close button */}
          <div className="overlay-header">
            <span className="overlay-title">// {label}</span>

            <div className="overlay-tabs" role="tablist" aria-label="Overlay tabs">
              <button
                type="button"
                role="tab"
                className="overlay-tab-btn"
                aria-selected={activeTab === 'chat'}
                aria-controls="overlay-panel-chat"
                onClick={() => setActiveTab('chat')}
              >
                <IconChat />
                Chat
                {msgCount > 0 && (
                  <span className="overlay-tab-badge" aria-label={`${msgCount} messages`}>
                    {msgCount > 99 ? '99+' : msgCount}
                  </span>
                )}
              </button>

              <button
                type="button"
                role="tab"
                className="overlay-tab-btn"
                aria-selected={activeTab === 'voice'}
                aria-controls="overlay-panel-voice"
                onClick={() => setActiveTab('voice')}
              >
                <IconVoice />
                Voice
                {participantCount > 0 && (
                  <span className="overlay-tab-badge" aria-label={`${participantCount} participants`}>
                    {participantCount}
                  </span>
                )}
              </button>
            </div>

            <button
              type="button"
              className="overlay-close-btn"
              aria-label="Close overlay"
              onClick={close}
            >
              <IconClose />
            </button>
          </div>

          {/* Chat tab panel */}
          <div
            id="overlay-panel-chat"
            style={
              activeTab === 'chat'
                ? { display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }
                : { display: 'none' }
            }
          >
            <ChatTab channelId={channelId} comms={comms} />
          </div>

          {/* Voice tab panel */}
          <div
            id="overlay-panel-voice"
            style={
              activeTab === 'voice'
                ? { display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }
                : { display: 'none' }
            }
          >
            <VoiceTab voiceRoomId={voiceRoomId} comms={comms} />
          </div>
        </div>
      )}
    </>
  );
}
