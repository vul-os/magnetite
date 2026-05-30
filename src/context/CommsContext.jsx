import { createContext, useContext, useState, useCallback, useMemo } from 'react';
import { useCommunities } from '../hooks/useCommunities';
import { useChannels } from '../hooks/useChannels';
import { useMessages } from '../hooks/useMessages';
import { usePresence } from '../hooks/usePresence';
import { useVoice } from '../hooks/useVoice';
import { useCommsSocket } from '../hooks/useCommsSocket';

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

const CommsContext = createContext(null);

export function CommsProvider({ children }) {
  // ── Selection state ───────────────────────────────────────────────────────
  const [activeCommunityId, setActiveCommunityId] = useState(null);
  const [activeChannelId, setActiveChannelId] = useState(null);
  const [activeDMUserId, setActiveDMUserId] = useState(null);

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
    (msg) => {
      // Real-time chat message → append if it's for the active channel
      if (msg.channel_id === activeChannelId) {
        appendMessage({ ...msg, pending: false });
      }
    },
    [activeChannelId, appendMessage]
  );

  const handlePresence = useCallback(
    (msg) => {
      setPresence(msg.user_id, { status: msg.status, activity: msg.activity ?? null });
    },
    [setPresence]
  );

  const handleVoiceState = useCallback(
    (msg) => {
      if (msg.action === 'join') addParticipant(msg.participant);
      else if (msg.action === 'leave') removeParticipant(msg.user_id);
      else if (msg.action === 'update') updateParticipant(msg.user_id, msg.updates);
    },
    [addParticipant, removeParticipant, updateParticipant]
  );

  const handleDM = useCallback(
    (msg) => {
      // If this DM is from the active DM user, append it
      if (activeDMUserId && msg.sender_id === activeDMUserId) {
        appendMessage({ ...msg, pending: false });
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
    (id) => {
      setActiveCommunityId(id);
      setActiveChannelId(null);
      setActiveDMUserId(null);
    },
    []
  );

  const selectChannel = useCallback((id) => {
    setActiveChannelId(id);
    setActiveDMUserId(null);
    setMessages([]);
  }, [setMessages]);

  const selectDM = useCallback((userId) => {
    setActiveDMUserId(userId);
    setActiveChannelId(null);
    setMessages([]);
  }, [setMessages]);

  // ── Join voice room + init WebRTC ─────────────────────────────────────────
  const joinVoiceRoom = useCallback(
    async (roomId, peerOpts = {}) => {
      const result = await joinRoom(roomId);
      if (result.success && result.token) {
        await initPeer(roomId, result.token, peerOpts);
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
  const value = useMemo(
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
export function useComms() {
  const context = useContext(CommsContext);
  if (!context) {
    throw new Error('useComms must be used within CommsProvider');
  }
  return context;
}
