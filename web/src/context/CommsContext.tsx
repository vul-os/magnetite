import { createContext, useContext, useState, useCallback, useMemo, type ReactNode } from 'react';
import { useCommunities } from '../hooks/useCommunities';
import { useChannels } from '../hooks/useChannels';
import { useMessages } from '../hooks/useMessages';
import { usePresence } from '../hooks/usePresence';
import { useVoice } from '../hooks/useVoice';
import { useCommsSocket, type PeerState, type InitPeerOptions } from '../hooks/useCommsSocket';
import type {
  WsFrame,
  Community,
  Channel,
  ChatMessage,
  PresenceMap,
  PresenceEntry,
  CommunityMember,
  VoiceRoom,
  VoiceParticipant,
  ActionResult,
} from '../types/comms';

export interface CommsContextValue {
  // Selection
  activeCommunityId: string | null;
  activeChannelId: string | null;
  activeDMUserId: string | null;
  activeCommunity: Community | null;
  activeChannel: Channel | null;
  selectCommunity: (id: string) => void;
  selectChannel: (id: string) => void;
  selectDM: (userId: string) => void;

  // Communities
  communities: Community[];
  communitiesLoading: boolean;
  // The underlying hook's async body sometimes returns a (never-consumed)
  // cleanup closure and sometimes returns nothing — see useCommunities.ts.
  fetchCommunities: () => Promise<(() => void) | undefined>;
  createCommunity: (data: { name: string; description?: string; icon_url?: string | null }) => Promise<ActionResult>;
  joinCommunity: (id: string) => Promise<ActionResult>;
  leaveCommunity: (id: string) => Promise<ActionResult>;

  // Channels
  channels: Channel[];
  textChannels: Channel[];
  voiceChannels: Channel[];
  channelsLoading: boolean;
  createChannel: (data: { name: string; kind?: Channel['kind'] }) => Promise<ActionResult>;

  // Messages
  messages: ChatMessage[];
  messagesLoading: boolean;
  hasMore: boolean;
  loadMore: () => void;
  postMessage: (content: string) => Promise<ActionResult>;
  appendMessage: (message: ChatMessage) => void;

  // Presence
  presenceMap: PresenceMap;
  getPresence: (userId: string) => PresenceEntry;
  bulkSetPresence: (snapshot: PresenceMap) => void;
  sortByPresence: <M extends CommunityMember>(members: M[]) => M[];
  onlineCount: number;

  // Voice
  voiceRooms: VoiceRoom[];
  currentRoom: VoiceRoom | null;
  joinToken: string | null;
  muted: boolean;
  deafened: boolean;
  voiceParticipants: VoiceParticipant[];
  fetchVoiceRooms: () => Promise<(() => void) | undefined>;
  joinVoiceRoom: (roomId: string, peerOpts?: InitPeerOptions) => Promise<ActionResult>;
  leaveVoiceRoom: () => void;
  toggleMute: () => void;
  toggleDeafen: () => void;

  // Realtime socket
  isConnected: boolean;
  voiceConnected: boolean;
  sendChatMessage: (content: string, targetChannelId?: string) => void;
  sendDMMessage: (content: string, recipientId: string) => void;
  sendTypingStart: () => void;
  sendTypingStop: () => void;
  typingUsers: Record<string, string>;

  // WebRTC
  peerState: PeerState;
  initPeer: (roomId: string, roomToken: string, opts?: InitPeerOptions) => Promise<{ localStream: MediaStream | null }>;
  destroyPeer: () => void;
}

/**
 * CommsContext — the single source of truth for Wave 6 comms state.
 *
 * Holds:
 *   - Current community selection
 *   - Current channel selection (text or voice)
 *   - Voice room state (joined room, mute/deafen, participants)
 *   - Presence map for the current community's members
 *   - Realtime socket connection state
 *
 * Usage:
 *   <CommsProvider>
 *     <YourApp />
 *   </CommsProvider>
 *
 *   const comms = useComms();
 */

const CommsContext = createContext<CommsContextValue | null>(null);

export function CommsProvider({ children }: { children: ReactNode }) {
  // ── Selection state ───────────────────────────────────────────────────────
  const [activeCommunityId, setActiveCommunityId] = useState<string | null>(null);
  const [activeChannelId, setActiveChannelId] = useState<string | null>(null);
  const [activeDMUserId, setActiveDMUserId] = useState<string | null>(null);

  // ── Communities ───────────────────────────────────────────────────────────
  const {
    communities,
    loading: communitiesLoading,
    fetchCommunities,
    createCommunity,
    joinCommunity,
    leaveCommunity,
  } = useCommunities();

  // ── Channels (for active community) ──────────────────────────────────────
  const {
    channels,
    textChannels,
    voiceChannels,
    loading: channelsLoading,
    createChannel,
  } = useChannels(activeCommunityId);

  // ── Voice ─────────────────────────────────────────────────────────────────
  const {
    rooms: voiceRooms,
    currentRoom,
    joinToken,
    muted,
    deafened,
    participants: voiceParticipants,
    fetchRooms: fetchVoiceRooms,
    joinRoom,
    leaveRoom,
    toggleMute,
    toggleDeafen,
    addParticipant,
    removeParticipant,
    updateParticipant,
  } = useVoice(activeCommunityId);

  // ── Presence ──────────────────────────────────────────────────────────────
  const { presenceMap, getPresence, setPresence, bulkSetPresence, sortByPresence, onlineCount } =
    usePresence();

  // ── Messages (for active channel or DM) ───────────────────────────────────
  const {
    messages,
    loading: messagesLoading,
    hasMore,
    loadMore,
    postMessage,
    appendMessage,
    setMessages,
  } = useMessages(activeChannelId, {
    isDM: !!activeDMUserId,
    dmUserId: activeDMUserId,
  });

  // ── Realtime socket ───────────────────────────────────────────────────────
  const handleMessage = useCallback(
    (msg: WsFrame) => {
      // Real-time chat message → append if it's for the active channel
      if (msg.channel_id === activeChannelId) {
        appendMessage({ ...msg, id: String(msg.id), content: typeof msg.content === 'string' ? msg.content : '', pending: false });
      }
    },
    [activeChannelId, appendMessage]
  );

  const handlePresence = useCallback(
    (msg: WsFrame) => {
      setPresence(String(msg.user_id), { status: msg.status as PresenceEntry['status'], activity: (msg.activity as string | null) ?? null });
    },
    [setPresence]
  );

  const handleVoiceState = useCallback(
    (msg: WsFrame) => {
      if (msg.action === 'join') addParticipant(msg.participant as VoiceParticipant);
      else if (msg.action === 'leave') removeParticipant(String(msg.user_id));
      else if (msg.action === 'update') updateParticipant(String(msg.user_id), msg.updates as Partial<VoiceParticipant>);
    },
    [addParticipant, removeParticipant, updateParticipant]
  );

  const handleDM = useCallback(
    (msg: WsFrame) => {
      // If this DM is from the active DM user, append it
      if (activeDMUserId && msg.sender_id === activeDMUserId) {
        appendMessage({ ...msg, id: String(msg.id), content: typeof msg.content === 'string' ? msg.content : '', pending: false });
      }
    },
    [activeDMUserId, appendMessage]
  );

  const {
    isConnected,
    voiceConnected,
    sendChatMessage,
    sendDMMessage,
    sendTypingStart,
    sendTypingStop,
    typingUsers,
    peerState,
    initPeer,
    destroyPeer,
  } = useCommsSocket({
    channelId: activeChannelId,
    communityId: activeCommunityId,
    onMessage: handleMessage,
    onPresence: handlePresence,
    onVoiceState: handleVoiceState,
    onDM: handleDM,
  });

  // ── Derived / selectors ───────────────────────────────────────────────────
  const activeCommunity = useMemo(
    () => communities.find((c) => c.id === activeCommunityId) ?? null,
    [communities, activeCommunityId]
  );

  const activeChannel = useMemo(
    () => channels.find((c) => c.id === activeChannelId) ?? null,
    [channels, activeChannelId]
  );

  // ── Navigation helpers ────────────────────────────────────────────────────
  const selectCommunity = useCallback(
    (id: string) => {
      setActiveCommunityId(id);
      setActiveChannelId(null);
      setActiveDMUserId(null);
    },
    []
  );

  const selectChannel = useCallback((id: string) => {
    setActiveChannelId(id);
    setActiveDMUserId(null);
    setMessages([]);
  }, [setMessages]);

  const selectDM = useCallback((userId: string) => {
    setActiveDMUserId(userId);
    setActiveChannelId(null);
    setMessages([]);
  }, [setMessages]);

  // ── Join voice room + init WebRTC ─────────────────────────────────────────
  const joinVoiceRoom = useCallback(
    async (roomId: string, peerOpts: InitPeerOptions = {}) => {
      const result = await joinRoom(roomId);
      if (result.success && result.token) {
        await initPeer(roomId, result.token as string, peerOpts);
      }
      return result;
    },
    [joinRoom, initPeer]
  );

  const leaveVoiceRoom = useCallback(() => {
    leaveRoom();
    destroyPeer();
  }, [leaveRoom, destroyPeer]);

  // ── Context value ─────────────────────────────────────────────────────────
  const value = useMemo<CommsContextValue>(
    () => ({
      // Selection
      activeCommunityId,
      activeChannelId,
      activeDMUserId,
      activeCommunity,
      activeChannel,
      selectCommunity,
      selectChannel,
      selectDM,

      // Communities
      communities,
      communitiesLoading,
      fetchCommunities,
      createCommunity,
      joinCommunity,
      leaveCommunity,

      // Channels
      channels,
      textChannels,
      voiceChannels,
      channelsLoading,
      createChannel,

      // Messages
      messages,
      messagesLoading,
      hasMore,
      loadMore,
      postMessage,
      appendMessage,

      // Presence
      presenceMap,
      getPresence,
      bulkSetPresence,
      sortByPresence,
      onlineCount,

      // Voice
      voiceRooms,
      currentRoom,
      joinToken,
      muted,
      deafened,
      voiceParticipants,
      fetchVoiceRooms,
      joinVoiceRoom,
      leaveVoiceRoom,
      toggleMute,
      toggleDeafen,

      // Realtime socket
      isConnected,
      voiceConnected,
      sendChatMessage,
      sendDMMessage,
      sendTypingStart,
      sendTypingStop,
      typingUsers,

      // WebRTC
      peerState,
      initPeer,
      destroyPeer,
    }),
    [
      activeCommunityId, activeChannelId, activeDMUserId, activeCommunity, activeChannel,
      selectCommunity, selectChannel, selectDM,
      communities, communitiesLoading, fetchCommunities, createCommunity, joinCommunity, leaveCommunity,
      channels, textChannels, voiceChannels, channelsLoading, createChannel,
      messages, messagesLoading, hasMore, loadMore, postMessage, appendMessage,
      presenceMap, getPresence, bulkSetPresence, sortByPresence, onlineCount,
      voiceRooms, currentRoom, joinToken, muted, deafened, voiceParticipants,
      fetchVoiceRooms, joinVoiceRoom, leaveVoiceRoom, toggleMute, toggleDeafen,
      isConnected, voiceConnected, sendChatMessage, sendDMMessage, sendTypingStart, sendTypingStop, typingUsers,
      peerState, initPeer, destroyPeer,
    ]
  );

  return <CommsContext.Provider value={value}>{children}</CommsContext.Provider>;
}

/**
 * useComms — consume the comms context.
 * Must be used inside <CommsProvider>.
 */
// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useComms() {
  const context = useContext(CommsContext);
  if (!context) {
    throw new Error('useComms must be used within CommsProvider');
  }
  return context;
}
