import { useState, useCallback, useEffect, useRef } from 'react';
import Navbar from '../components/Navbar';
import ServerRail from '../components/comms/ServerRail';
import ChannelList from '../components/comms/ChannelList';
import MessageList from '../components/comms/MessageList';
import MessageComposer from '../components/comms/MessageComposer';
import MemberList from '../components/comms/MemberList';
import VoicePanel from '../components/comms/VoicePanel';
import { useComms } from '../context/CommsContext';
import { useCommunityMembers } from '../hooks/useCommunities';
import { useAuth } from '../hooks/useAuth';
import './Communities.css';

// ─── Normalise API data shapes to what the visual components expect ───────────

/** Channels from the API use `kind`; components expect `type`. */
function normaliseChannel(ch) {
  if (!ch) return ch;
  return { ...ch, type: ch.type ?? ch.kind ?? 'text' };
}

/** Messages from the API use nested `author` obj; MessageList expects flat fields. */
function normaliseMessage(msg) {
  if (!msg) return msg;
  if (typeof msg.author === 'object' && msg.author !== null) {
    return {
      ...msg,
      authorId: msg.author.id ?? msg.author_id ?? null,
      author: msg.author.display_name ?? msg.author.username ?? 'Unknown',
      createdAt: msg.created_at ?? msg.createdAt ?? new Date().toISOString(),
    };
  }
  // Already flat (mock fallback)
  return { ...msg, createdAt: msg.created_at ?? msg.createdAt ?? new Date().toISOString() };
}

/** Members from the API have `display_name`; MemberList uses `username`. */
function normaliseMember(m) {
  if (!m) return m;
  return {
    ...m,
    username: m.display_name ?? m.username ?? 'Unknown',
    status: m.status ?? 'offline',
    game: m.activity ?? m.game ?? null,
  };
}

// ─── Typing indicator banner ──────────────────────────────────────────────────

function TypingBanner({ typingUsers }) {
  const names = Object.values(typingUsers ?? {});
  if (names.length === 0) return null;

  let text;
  if (names.length === 1) text = `${names[0]} is typing…`;
  else if (names.length === 2) text = `${names[0]} and ${names[1]} are typing…`;
  else text = 'Several people are typing…';

  return (
    <div className="typing-banner" aria-live="polite" aria-atomic="true">
      <span className="typing-dots" aria-hidden="true">
        <span className="typing-dot" />
        <span className="typing-dot" />
        <span className="typing-dot" />
      </span>
      <span className="typing-banner__text">{text}</span>
    </div>
  );
}

// ─── Connection status pill ───────────────────────────────────────────────────

function ConnectionStatus({ isConnected }) {
  return (
    <span
      className={`conn-status ${isConnected ? 'conn-status--online' : 'conn-status--offline'}`}
      aria-label={isConnected ? 'Connected to comms server' : 'Connecting to comms server…'}
      title={isConnected ? 'Live' : 'Connecting…'}
    >
      <span className="conn-status__dot" aria-hidden="true" />
      {isConnected ? 'Live' : 'Connecting…'}
    </span>
  );
}

// ─── Loading skeleton ─────────────────────────────────────────────────────────

function CommunitiesSkeleton() {
  return (
    <div className="communities-page" aria-busy="true" aria-label="Loading communities…">
      <div className="communities-shell bg-atmosphere">
        <div className="communities-skeleton__rail" aria-hidden="true">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="communities-skeleton__icon shimmer" />
          ))}
        </div>
        <div className="communities-skeleton__channels" aria-hidden="true">
          {[...Array(6)].map((_, i) => (
            <div key={i} className="communities-skeleton__channel shimmer" />
          ))}
        </div>
        <div className="communities-skeleton__main" aria-hidden="true">
          {[...Array(4)].map((_, i) => (
            <div key={i} className="communities-skeleton__message shimmer" />
          ))}
        </div>
      </div>
    </div>
  );
}

// ─── Empty state when no communities ─────────────────────────────────────────

function NoCommunities({ onCreate }) {
  return (
    <div className="communities-empty" role="region" aria-label="No communities">
      <div className="communities-empty__inner">
        <div className="communities-empty__icon" aria-hidden="true">
          <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
            <circle cx="9" cy="7" r="4" />
            <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
            <path d="M16 3.13a4 4 0 0 1 0 7.75" />
          </svg>
        </div>
        <span className="kicker">// no communities yet</span>
        <h2 className="communities-empty__title">Join a community</h2>
        <p className="communities-empty__desc">
          Connect with other Rust game developers. Create your own community or browse
          public ones.
        </p>
        <button className="btn btn-primary" onClick={onCreate} aria-label="Create a new community">
          Create Community
        </button>
      </div>
    </div>
  );
}

// ─── Main component ────────────────────────────────────────────────────────────

export default function Communities() {
  // Real user from auth; fall back to a stable guest ID if not logged in.
  const { user } = useAuth();
  const currentUserId = user?.id ? String(user.id) : 'guest';

  const {
    // Communities
    communities,
    communitiesLoading,
    activeCommunityId,
    activeCommunity,
    selectCommunity,

    // Channels
    channels: rawChannels,
    textChannels: rawTextChannels,
    channelsLoading,
    activeChannelId,
    activeChannel: rawActiveChannel,
    selectChannel,

    // Messages
    messages: rawMessages,
    messagesLoading,
    hasMore,
    loadMore,
    postMessage,

    // Presence
    presenceMap,
    getPresence,
    sortByPresence,
    onlineCount,

    // Voice
    currentRoom,
    muted,
    deafened,
    voiceParticipants,
    joinVoiceRoom,
    leaveVoiceRoom,
    toggleMute,
    toggleDeafen,

    // Socket
    isConnected,
    sendChatMessage,
    sendTypingStart,
    sendTypingStop,
    typingUsers,
  } = useComms();

  // Per-community members
  const { members: rawMembers } = useCommunityMembers(activeCommunityId);

  // Local voice channel selection (for UI only — voice room join is async)
  const [selectedVoiceChannelId, setSelectedVoiceChannelId] = useState(null);

  // Normalise data shapes
  const channels = rawChannels.map(normaliseChannel);
  const textChannels = rawTextChannels.map(normaliseChannel);
  const activeChannel = rawActiveChannel ? normaliseChannel(rawActiveChannel) : null;
  const messages = rawMessages.map(normaliseMessage);

  // Enrich members with presence data then normalise
  const members = sortByPresence(rawMembers).map((m) => {
    const presence = getPresence(m.id);
    return normaliseMember({ ...m, status: presence.status, activity: presence.activity });
  });

  // Auto-select first community on load
  useEffect(() => {
    if (!activeCommunityId && communities.length > 0) {
      selectCommunity(communities[0].id);
    }
  }, [communities, activeCommunityId, selectCommunity]);

  // Auto-select first text channel when community changes
  useEffect(() => {
    if (activeCommunityId && textChannels.length > 0 && !activeChannelId && !selectedVoiceChannelId) {
      selectChannel(textChannels[0].id);
    }
  }, [activeCommunityId, textChannels, activeChannelId, selectedVoiceChannelId, selectChannel]);

  // Map communities → ServerRail shape; add online_count from presence
  const servers = communities.map((c) => ({
    id: c.id,
    name: c.name,
    icon: c.icon_url ?? null,
    color: null,
    unread: 0,
  }));

  const handleSelectServer = useCallback((id) => {
    if (id === '__home') return; // DMs — handled by social agent's /messages route
    selectCommunity(id);
    setSelectedVoiceChannelId(null);
  }, [selectCommunity]);

  const handleSelectChannel = useCallback((channel) => {
    const norm = normaliseChannel(channel);
    if (norm.type === 'voice') {
      setSelectedVoiceChannelId(norm.id);
      // Don't select as text channel
    } else {
      setSelectedVoiceChannelId(null);
      selectChannel(norm.id);
    }
  }, [selectChannel]);

  const handleSend = useCallback(async (text) => {
    if (!text.trim()) return;
    // Send over socket for real-time broadcast + persist via REST
    sendChatMessage(text);
    await postMessage(text);
  }, [sendChatMessage, postMessage]);

  const handleJoinVoice = useCallback(async () => {
    if (!selectedVoiceChannelId) return;
    await joinVoiceRoom(selectedVoiceChannelId);
  }, [selectedVoiceChannelId, joinVoiceRoom]);

  const handleLeaveVoice = useCallback(() => {
    leaveVoiceRoom();
    setSelectedVoiceChannelId(null);
  }, [leaveVoiceRoom]);

  // Scroll-to-load-more ref
  const topRef = useRef(null);

  // Loading state
  if (communitiesLoading && communities.length === 0) {
    return (
      <>
        <Navbar />
        <CommunitiesSkeleton />
      </>
    );
  }

  // No communities
  if (!communitiesLoading && communities.length === 0) {
    return (
      <div className="communities-page">
        <Navbar />
        <main id="main-content" className="communities-shell bg-atmosphere">
          <NoCommunities onCreate={() => {}} />
        </main>
      </div>
    );
  }

  const isTextChannel = activeChannel?.type === 'text' || (activeChannelId && !selectedVoiceChannelId);
  const selectedVoiceChannel = channels.find((c) => c.id === selectedVoiceChannelId) ?? null;
  const inVoiceRoom = !!currentRoom;

  // Build voice participants from context (real-time) or mock from channel
  const vParticipants = inVoiceRoom && voiceParticipants.length > 0
    ? voiceParticipants
    : (selectedVoiceChannel?.participants ?? []);

  return (
    <div className="communities-page">
      <Navbar />

      <main
        id="main-content"
        className="communities-shell bg-atmosphere"
        aria-label="Communities"
      >
        {/* 1. Server rail */}
        <ServerRail
          servers={servers}
          activeId={activeCommunityId}
          onSelect={handleSelectServer}
        />

        {/* 2. Channel list */}
        <ChannelList
          server={activeCommunity ?? null}
          channels={channels}
          activeChannelId={isTextChannel ? activeChannelId : null}
          onSelect={handleSelectChannel}
          loading={channelsLoading}
        />

        {/* 3. Main area */}
        <div className="communities-main">
          {/* Channel header */}
          <header className="channel-header" aria-label="Channel information">
            <div className="channel-header__left">
              <span className="channel-header__hash" aria-hidden="true">
                {isTextChannel ? (
                  <svg width="20" height="20" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path d="M6.5 2.5L5 13.5M11 2.5L9.5 13.5M2.5 6h11M2 10h11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                  </svg>
                ) : (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
                    <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
                    <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
                  </svg>
                )}
              </span>
              <h1 className="channel-header__name">
                {activeChannel?.name ?? selectedVoiceChannel?.name ?? 'Select a channel'}
              </h1>
              {(activeChannel || selectedVoiceChannel) && (
                <p className="channel-header__topic">
                  {isTextChannel
                    ? (activeChannel?.topic ?? activeCommunity?.description ?? 'Welcome!')
                    : `Voice channel · ${vParticipants.length} connected`}
                </p>
              )}
            </div>

            <div className="channel-header__right">
              <ConnectionStatus isConnected={isConnected} />
              {onlineCount > 0 && (
                <span className="channel-header__online kicker" aria-label={`${onlineCount} members online`}>
                  {onlineCount} online
                </span>
              )}
              <button className="channel-header-btn" aria-label="Search in channel">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <circle cx="11" cy="11" r="8" />
                  <line x1="21" y1="21" x2="16.65" y2="16.65" />
                </svg>
              </button>
              <button className="channel-header-btn" aria-label="Pinned messages">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <line x1="12" y1="17" x2="12" y2="22" />
                  <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24z" />
                </svg>
              </button>
              <button className="channel-header-btn" aria-label="Toggle member list">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
                  <circle cx="9" cy="7" r="4" />
                  <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
                  <path d="M16 3.13a4 4 0 0 1 0 7.75" />
                </svg>
              </button>
            </div>
          </header>

          {/* Active voice panel (shown above chat when in voice room) */}
          {inVoiceRoom && currentRoom && (
            <VoicePanel
              channel={currentRoom}
              participants={vParticipants}
              onLeave={handleLeaveVoice}
              muted={muted}
              deafened={deafened}
              onToggleMute={toggleMute}
              onToggleDeafen={toggleDeafen}
            />
          )}

          {/* Message area (text channel) */}
          {isTextChannel ? (
            <>
              {/* Load-more trigger */}
              {hasMore && (
                <div ref={topRef} className="load-more-bar">
                  <button
                    className="btn btn-ghost load-more-btn"
                    onClick={loadMore}
                    disabled={messagesLoading}
                    aria-label="Load older messages"
                  >
                    {messagesLoading ? 'Loading…' : 'Load older messages'}
                  </button>
                </div>
              )}

              <MessageList
                messages={messages}
                currentUserId={currentUserId}
                loading={messagesLoading && messages.length === 0}
              />

              <TypingBanner typingUsers={typingUsers} />

              <MessageComposer
                channel={activeChannel}
                onSend={handleSend}
                onTypingStart={sendTypingStart}
                onTypingStop={sendTypingStop}
                disabled={!activeChannelId}
              />
            </>
          ) : (
            /* Voice channel selected — not yet joined */
            <div className="voice-channel-view" role="region" aria-label="Voice channel view">
              <div className="voice-channel-view__inner">
                <span className="kicker">// voice channel</span>
                <h2 className="voice-channel-view__title">
                  {selectedVoiceChannel?.name ?? 'Voice Channel'}
                </h2>
                <p className="voice-channel-view__desc">
                  Join to connect with others via WebRTC voice. Peer-to-peer mesh for
                  small rooms; SFU-ready for scale.
                </p>
                <button
                  className="btn btn-primary voice-channel-view__join"
                  onClick={handleJoinVoice}
                  aria-label={`Join ${selectedVoiceChannel?.name ?? 'voice channel'}`}
                >
                  Join Voice
                </button>
              </div>
            </div>
          )}
        </div>

        {/* 4. Member list */}
        <MemberList
          members={members}
          currentUserId={currentUserId}
          presenceMap={presenceMap}
        />
      </main>
    </div>
  );
}
