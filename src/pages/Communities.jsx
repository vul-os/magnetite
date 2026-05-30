import { useState, useCallback } from 'react';
import Navbar from '../components/Navbar';
import ServerRail from '../components/comms/ServerRail';
import ChannelList from '../components/comms/ChannelList';
import MessageList from '../components/comms/MessageList';
import MessageComposer from '../components/comms/MessageComposer';
import MemberList from '../components/comms/MemberList';
import VoicePanel from '../components/comms/VoicePanel';
import './Communities.css';

// ─── Mock data (Wave 7 will replace with real comms hooks) ────────────────────

const MOCK_SERVERS = [
  {
    id: 'srv-1',
    name: 'Magnetite Dev',
    color: '#38e1c8',
    unread: 3,
  },
  {
    id: 'srv-2',
    name: 'FPS Builders',
    color: '#5b9dff',
    unread: 0,
  },
  {
    id: 'srv-3',
    name: 'Bevy Engine',
    color: '#f5a524',
    unread: 12,
  },
  {
    id: 'srv-4',
    name: 'Rust Gamedev',
    color: '#ff5468',
    unread: 0,
  },
  {
    id: 'srv-5',
    name: 'Rapier Physics',
    color: '#3ddc84',
    unread: 1,
  },
];

const MOCK_CHANNELS = {
  'srv-1': [
    { id: 'ch-1', name: 'general',       type: 'text',  unread: 2 },
    { id: 'ch-2', name: 'announcements', type: 'text',  unread: 1, private: false },
    { id: 'ch-3', name: 'sdk-dev',       type: 'text',  unread: 0 },
    { id: 'ch-4', name: 'backend',       type: 'text',  unread: 0 },
    { id: 'ch-5', name: 'frontend',      type: 'text',  unread: 0 },
    { id: 'ch-6', name: 'beta-testing',  type: 'text',  unread: 0, private: true },
    {
      id: 'ch-v1',
      name: 'General Voice',
      type: 'voice',
      participants: [
        { id: 'u-1', username: 'rustdev42',  muted: false, status: 'online' },
        { id: 'u-2', username: 'bevy_fan',   muted: true,  status: 'online' },
      ],
    },
    {
      id: 'ch-v2',
      name: 'Pair Programming',
      type: 'voice',
      participants: [],
    },
  ],
  'srv-2': [
    { id: 'ch-7',  name: 'general',       type: 'text', unread: 0 },
    { id: 'ch-8',  name: 'fps-showcase',  type: 'text', unread: 0 },
    { id: 'ch-v3', name: 'Playtest Voice', type: 'voice', participants: [] },
  ],
};

const MOCK_MESSAGES = {
  'ch-1': [
    {
      id: 'm-1',
      authorId: 'u-100',
      author: 'imranparuk',
      content: 'Welcome to #general! This is the home of the Magnetite Dev community.',
      createdAt: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
      role: 'admin',
      reactions: [{ emoji: '🎉', count: 4, label: 'Party', userReacted: false }],
    },
    {
      id: 'm-2',
      authorId: 'u-1',
      author: 'rustdev42',
      content: 'Just shipped the WebRTC signaling layer on the backend. Voice rooms are ready for the frontend wave!',
      createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
      reactions: [
        { emoji: '🔥', count: 7, label: 'Fire', userReacted: true },
        { emoji: '🦀', count: 3, label: 'Crab', userReacted: false },
      ],
    },
    {
      id: 'm-3',
      authorId: 'u-1',
      author: 'rustdev42',
      content: 'The SDP/ICE relay is implemented as a WS handler. Mesh for ≤6 peers, SFU path documented for scale.',
      createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000 + 45000).toISOString(),
    },
    {
      id: 'm-4',
      authorId: 'u-2',
      author: 'bevy_fan',
      content: 'Amazing work! The communities migration SQL looks clean. When does the frontend wire up?',
      createdAt: new Date(Date.now() - 1 * 60 * 60 * 1000).toISOString(),
    },
    {
      id: 'm-5',
      authorId: 'u-3',
      author: 'ferris_builds',
      content: 'Wave 7 will wire the comms hooks. This wave (Wave 6) is the UI shell with mock data.',
      createdAt: new Date(Date.now() - 45 * 60 * 1000).toISOString(),
    },
    {
      id: 'm-6',
      authorId: 'u-3',
      author: 'ferris_builds',
      content: 'Communities page is live: server rail, channel list, chat, voice panel, member list — all with the Industrial Magnetite design system.',
      createdAt: new Date(Date.now() - 44 * 60 * 1000).toISOString(),
      reactions: [
        { emoji: '⚡', count: 5, label: 'Zap', userReacted: false },
      ],
    },
    {
      id: 'm-7',
      authorId: 'u-4',
      author: 'game_dev_mx',
      content: 'The server rail magnetic hover is chef\'s kiss. Very Discord-like but distinctly Magnetite.',
      createdAt: new Date(Date.now() - 30 * 60 * 1000).toISOString(),
    },
    {
      id: 'm-8',
      authorId: 'u-100',
      author: 'imranparuk',
      content: 'Exactly the goal. This is still a shell — Wave 7 wires real WebSocket presence and voice signaling. For now, mock data proves the layout and UX.',
      createdAt: new Date(Date.now() - 10 * 60 * 1000).toISOString(),
      role: 'admin',
    },
  ],
  'ch-2': [
    {
      id: 'ann-1',
      authorId: 'u-100',
      author: 'imranparuk',
      content: '📢 Wave 6 of the autonomous Magnetite build is underway. Comms core: communities, channels, messages, presence, voice signaling.',
      createdAt: new Date(Date.now() - 4 * 60 * 60 * 1000).toISOString(),
      role: 'admin',
    },
  ],
};

const MOCK_MEMBERS = [
  { id: 'u-100', username: 'imranparuk',   status: 'online',  game: null },
  { id: 'u-1',   username: 'rustdev42',    status: 'online',  game: 'FPS Starter' },
  { id: 'u-2',   username: 'bevy_fan',     status: 'idle',    game: null },
  { id: 'u-3',   username: 'ferris_builds', status: 'online', game: null },
  { id: 'u-4',   username: 'game_dev_mx',  status: 'dnd',     game: 'Motorsport Demo' },
  { id: 'u-5',   username: 'wasm_wizard',  status: 'online',  game: null },
  { id: 'u-6',   username: 'async_alice',  status: 'offline', game: null },
  { id: 'u-7',   username: 'rapier_pete',  status: 'offline', game: null },
];

const MOCK_VOICE_PARTICIPANTS = [
  { id: 'u-1', username: 'rustdev42', status: 'online', muted: false, speaking: true },
  { id: 'u-2', username: 'bevy_fan',  status: 'online', muted: true,  speaking: false },
];

// Current user (mock — in production pulled from auth context)
const CURRENT_USER_ID = 'u-100';

// ─── Component ────────────────────────────────────────────────────────────────

export default function Communities() {
  const [activeServerId,  setActiveServerId]  = useState('srv-1');
  const [activeChannelId, setActiveChannelId] = useState('ch-1');
  const [messages,        setMessages]        = useState(MOCK_MESSAGES);
  const [inVoiceChannel,  setInVoiceChannel]  = useState(null); // null or channel obj

  // Derived
  const activeServer  = MOCK_SERVERS.find(s => s.id === activeServerId);
  const serverChannels = MOCK_CHANNELS[activeServerId] ?? [];
  const activeChannel = serverChannels.find(c => c.id === activeChannelId);
  const channelMessages = messages[activeChannelId] ?? [];
  const activeVoiceChannel = serverChannels.find(c => c.id === inVoiceChannel);

  const handleSelectServer = useCallback((id) => {
    setActiveServerId(id);
    // Switch to first text channel of new server
    const firstText = (MOCK_CHANNELS[id] ?? []).find(c => c.type === 'text');
    if (firstText) setActiveChannelId(firstText.id);
    setInVoiceChannel(null);
  }, []);

  const handleSelectChannel = useCallback((channel) => {
    if (channel.type === 'voice') {
      setInVoiceChannel(channel.id);
    } else {
      setActiveChannelId(channel.id);
    }
  }, []);

  const handleSend = useCallback((text) => {
    const newMsg = {
      id: `msg-${Date.now()}`,
      authorId: CURRENT_USER_ID,
      author: 'imranparuk',
      content: text,
      createdAt: new Date().toISOString(),
      role: 'admin',
    };
    setMessages(prev => ({
      ...prev,
      [activeChannelId]: [...(prev[activeChannelId] ?? []), newMsg],
    }));
  }, [activeChannelId]);

  const handleLeaveVoice = useCallback(() => {
    setInVoiceChannel(null);
  }, []);

  const isTextChannel = activeChannel?.type === 'text';

  return (
    <div className="communities-page">
      {/* Top nav */}
      <Navbar />

      {/* Three-pane shell */}
      <main
        id="main-content"
        className="communities-shell bg-atmosphere"
        aria-label="Communities"
      >
        {/* 1. Server rail */}
        <ServerRail
          servers={MOCK_SERVERS}
          activeId={activeServerId}
          onSelect={handleSelectServer}
        />

        {/* 2. Channel list */}
        <ChannelList
          server={activeServer}
          channels={serverChannels}
          activeChannelId={isTextChannel ? activeChannelId : null}
          onSelect={handleSelectChannel}
        />

        {/* 3. Main area: chat */}
        <div className="communities-main">
          {/* Channel header */}
          <header className="channel-header" aria-label="Channel information">
            <div className="channel-header__left">
              <span className="channel-header__hash" aria-hidden="true">
                {isTextChannel ? (
                  <svg width="20" height="20" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path d="M6.5 2.5L5 13.5M11 2.5L9.5 13.5M2.5 6h11M2 10h11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                  </svg>
                ) : (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
                    <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
                  </svg>
                )}
              </span>
              <h1 className="channel-header__name">
                {activeChannel?.name ?? 'Select a channel'}
              </h1>
              {activeChannel?.name && (
                <p className="channel-header__topic">
                  {isTextChannel
                    ? 'Wave 6 comms core — real data arrives in Wave 7'
                    : `Voice channel · ${activeChannel.participants?.length ?? 0} connected`
                  }
                </p>
              )}
            </div>

            <div className="channel-header__right">
              {/* Search within channel */}
              <button className="channel-header-btn" aria-label="Search in channel">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
                </svg>
              </button>
              {/* Pin messages */}
              <button className="channel-header-btn" aria-label="Pinned messages">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24z"/>
                </svg>
              </button>
              {/* Member list toggle */}
              <button className="channel-header-btn" aria-label="Toggle member list">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
                  <circle cx="9" cy="7" r="4"/>
                  <path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>
                </svg>
              </button>
            </div>
          </header>

          {/* Active voice panel (shown above chat when in a voice channel) */}
          {inVoiceChannel && activeVoiceChannel && (
            <VoicePanel
              channel={activeVoiceChannel}
              participants={MOCK_VOICE_PARTICIPANTS}
              onLeave={handleLeaveVoice}
            />
          )}

          {/* Message area */}
          {isTextChannel ? (
            <>
              <MessageList
                messages={channelMessages}
                currentUserId={CURRENT_USER_ID}
              />
              <MessageComposer
                channel={activeChannel}
                onSend={handleSend}
              />
            </>
          ) : (
            /* Voice channel selected — show connected UI */
            <div className="voice-channel-view" role="region" aria-label="Voice channel view">
              <div className="voice-channel-view__inner">
                <span className="kicker">// voice channel</span>
                <h2 className="voice-channel-view__title">
                  {activeChannel?.name}
                </h2>
                <p className="voice-channel-view__desc">
                  Click &ldquo;Join Voice&rdquo; to connect. WebRTC signaling is ready —
                  peer-to-peer voice connects in Wave 7.
                </p>
                <button
                  className="btn btn-primary voice-channel-view__join"
                  onClick={() => setInVoiceChannel(activeChannelId)}
                  aria-label={`Join ${activeChannel?.name} voice channel`}
                >
                  Join Voice
                </button>
              </div>
            </div>
          )}
        </div>

        {/* 4. Member list */}
        <MemberList
          members={MOCK_MEMBERS}
          currentUserId={CURRENT_USER_ID}
        />
      </main>
    </div>
  );
}
