import { useState, useEffect, useCallback, useRef } from 'react';
import { useWebSocket } from './useWebSocket';

/**
 * useCommsSocket — the realtime backbone for Wave 6 comms.
 *
 * Architecture (per DECISIONS.md §4b):
 *  - Text chat + presence: Axum WebSocket at /ws/comms
 *  - Voice signaling: WebRTC SDP/ICE relay over /ws/voice
 *    Mesh for small rooms; SFU (LiveKit/mediasoup) is the scale path.
 *
 * Message envelope: { type, ...payload }
 *
 * Inbound types handled:
 *   chat_message       — new chat message in a channel
 *   presence_update    — user came online/offline/idle
 *   voice_state_update — participant joined/left/muted in a voice room
 *   sdp_offer          — WebRTC SDP offer relayed from a peer
 *   sdp_answer         — WebRTC SDP answer relayed from a peer
 *   ice_candidate      — ICE candidate relayed from a peer
 *   typing_start/stop  — typing indicator
 *   dm_message         — direct message
 *
 * Outbound helpers exposed: sendChatMessage, sendDM, sendTyping,
 *   sendVoiceSignal (SDP/ICE relay)
 *
 * WebRTC peer helper (minimal mesh for small rooms):
 *   initPeer(roomId, token, { onTrack, onStateChange })
 *   → starts getUserMedia, creates RTCPeerConnection, relays SDP/ICE via WS
 *   destroyPeer()
 */

const WS_COMMS_PATH = '/ws/comms';
const WS_VOICE_PATH = '/ws/voice';

const ICE_SERVERS = [{ urls: 'stun:stun.l.google.com:19302' }];

// ── helpers ────────────────────────────────────────────────────────────────
function noop() {}

// ── hook ───────────────────────────────────────────────────────────────────
export function useCommsSocket({
  channelId = null,
  communityId = null,
  onMessage = noop,
  onPresence = noop,
  onVoiceState = noop,
  onDM = noop,
} = {}) {
  // ── Chat/presence WS ─────────────────────────────────────────────────────
  const { isConnected, lastMessage, sendMessage } = useWebSocket(WS_COMMS_PATH, {
    autoReconnect: true,
    heartbeatInterval: 25_000,
    reconnectDelay: 3_000,
  });

  // ── Voice signaling WS ───────────────────────────────────────────────────
  const {
    isConnected: voiceConnected,
    lastMessage: voiceLastMessage,
    sendMessage: sendVoiceSignal,
  } = useWebSocket(WS_VOICE_PATH, {
    autoReconnect: true,
    heartbeatInterval: 25_000,
    reconnectDelay: 3_000,
  });

  // ── Typing state ─────────────────────────────────────────────────────────
  const [typingUsers, setTypingUsers] = useState({});
  const typingTimersRef = useRef({});

  // ── WebRTC peer state ─────────────────────────────────────────────────────
  const [peerState, setPeerState] = useState('idle'); // idle | connecting | connected | failed | closed
  const pcRef = useRef(null);
  const localStreamRef = useRef(null);

  // ── Inbound: chat/presence WS messages ───────────────────────────────────
  useEffect(() => {
    if (!lastMessage) return;
    const msg = lastMessage;

    switch (msg.type) {
      case 'chat_message':
        onMessage(msg);
        break;

      case 'dm_message':
        onDM(msg);
        break;

      case 'presence_update':
        onPresence(msg);
        break;

      case 'voice_state_update':
        onVoiceState(msg);
        break;

      case 'typing_start': {
        const uid = msg.user_id;
        if (typingTimersRef.current[uid]) clearTimeout(typingTimersRef.current[uid]);
        setTypingUsers((prev) => ({ ...prev, [uid]: msg.username ?? uid }));
        typingTimersRef.current[uid] = setTimeout(() => {
          setTypingUsers((prev) => {
            const next = { ...prev };
            delete next[uid];
            return next;
          });
        }, 5_000);
        break;
      }

      case 'typing_stop': {
        const uid = msg.user_id;
        if (typingTimersRef.current[uid]) clearTimeout(typingTimersRef.current[uid]);
        setTypingUsers((prev) => {
          const next = { ...prev };
          delete next[uid];
          return next;
        });
        break;
      }

      default:
        break;
    }
  }, [lastMessage, onMessage, onPresence, onVoiceState, onDM]);

  // ── Inbound: voice signaling WS messages ─────────────────────────────────
  useEffect(() => {
    if (!voiceLastMessage || !pcRef.current) return;
    const msg = voiceLastMessage;

    (async () => {
      const pc = pcRef.current;
      if (!pc) return;
      try {
        if (msg.type === 'sdp_offer') {
          await pc.setRemoteDescription(new RTCSessionDescription({ type: 'offer', sdp: msg.sdp }));
          const answer = await pc.createAnswer();
          await pc.setLocalDescription(answer);
          sendVoiceSignal({ type: 'sdp_answer', sdp: answer.sdp, room_id: msg.room_id });
        } else if (msg.type === 'sdp_answer') {
          await pc.setRemoteDescription(new RTCSessionDescription({ type: 'answer', sdp: msg.sdp }));
        } else if (msg.type === 'ice_candidate') {
          await pc.addIceCandidate(new RTCIceCandidate(msg.candidate));
        }
      } catch {
        // Silently swallow — peer may have left
      }
    })();
  }, [voiceLastMessage, sendVoiceSignal]);

  // Notify backend which channel we're listening to when it changes
  useEffect(() => {
    if (!isConnected || !channelId) return;
    sendMessage({ type: 'subscribe_channel', channel_id: channelId, community_id: communityId });
  }, [isConnected, channelId, communityId, sendMessage]);

  // Cleanup typing timers on unmount
  useEffect(() => {
    return () => {
      Object.values(typingTimersRef.current).forEach(clearTimeout);
    };
  }, []);

  // ── Outbound helpers ──────────────────────────────────────────────────────
  const sendChatMessage = useCallback(
    (content, targetChannelId) => {
      sendMessage({
        type: 'chat_message',
        channel_id: targetChannelId ?? channelId,
        content,
      });
    },
    [sendMessage, channelId]
  );

  const sendDMMessage = useCallback(
    (content, recipientId) => {
      sendMessage({ type: 'dm_message', recipient_id: recipientId, content });
    },
    [sendMessage]
  );

  const sendTypingStart = useCallback(() => {
    if (!channelId) return;
    sendMessage({ type: 'typing_start', channel_id: channelId });
  }, [sendMessage, channelId]);

  const sendTypingStop = useCallback(() => {
    if (!channelId) return;
    sendMessage({ type: 'typing_stop', channel_id: channelId });
  }, [sendMessage, channelId]);

  // ── WebRTC peer helper ────────────────────────────────────────────────────
  /**
   * Initialise a WebRTC peer connection for a voice room.
   *
   * @param {string} roomId   - voice room id
   * @param {string} token    - join token from api.voice.joinToken()
   * @param {object} opts
   *   onTrack(stream)        - called when a remote track is received
   *   onStateChange(state)   - called when connection state changes
   *   audio {boolean}        - request mic (default true)
   *   video {boolean}        - request camera (default false)
   */
  const initPeer = useCallback(
    async (roomId, token, { onTrack = noop, onStateChange = noop, audio = true, video = false } = {}) => {
      // Destroy any existing peer first
      if (pcRef.current) {
        pcRef.current.close();
        pcRef.current = null;
      }
      if (localStreamRef.current) {
        localStreamRef.current.getTracks().forEach((t) => t.stop());
        localStreamRef.current = null;
      }

      setPeerState('connecting');

      let localStream = null;
      try {
        if (typeof navigator !== 'undefined' && navigator.mediaDevices) {
          localStream = await navigator.mediaDevices.getUserMedia({ audio, video });
        }
      } catch {
        // Mic/camera permission denied or not available — continue without local media
      }
      localStreamRef.current = localStream;

      const pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
      pcRef.current = pc;

      // Add local tracks
      if (localStream) {
        localStream.getTracks().forEach((track) => pc.addTrack(track, localStream));
      }

      // Remote track received
      pc.ontrack = (event) => {
        const [remoteStream] = event.streams;
        onTrack(remoteStream);
      };

      // ICE candidate → relay over voice WS
      pc.onicecandidate = (event) => {
        if (event.candidate) {
          sendVoiceSignal({
            type: 'ice_candidate',
            candidate: event.candidate.toJSON(),
            room_id: roomId,
          });
        }
      };

      // Connection state
      pc.onconnectionstatechange = () => {
        const s = pc.connectionState;
        setPeerState(s);
        onStateChange(s);
      };

      // Send join intent over voice WS; backend sends back SDP offer for mesh peers
      sendVoiceSignal({ type: 'voice_join', room_id: roomId, token });

      // As offerer: create offer (sent to new peers arriving AFTER us)
      try {
        const offer = await pc.createOffer({ offerToReceiveAudio: true, offerToReceiveVideo: video });
        await pc.setLocalDescription(offer);
        sendVoiceSignal({ type: 'sdp_offer', sdp: offer.sdp, room_id: roomId });
      } catch {
        // Peer may not be ready yet; answerer path handles the other direction
      }

      return { localStream };
    },
    [sendVoiceSignal]
  );

  /** Tear down the current WebRTC peer connection and release mic/camera. */
  const destroyPeer = useCallback(() => {
    if (pcRef.current) {
      pcRef.current.close();
      pcRef.current = null;
    }
    if (localStreamRef.current) {
      localStreamRef.current.getTracks().forEach((t) => t.stop());
      localStreamRef.current = null;
    }
    setPeerState('idle');
  }, []);

  // ── Cleanup on unmount ────────────────────────────────────────────────────
  useEffect(() => {
    return () => {
      destroyPeer();
    };
  }, [destroyPeer]);

  return {
    // Connection state
    isConnected,
    voiceConnected,
    // Outbound
    sendChatMessage,
    sendDMMessage,
    sendTypingStart,
    sendTypingStop,
    sendVoiceSignal,
    // Typing
    typingUsers,
    // WebRTC
    peerState,
    initPeer,
    destroyPeer,
  };
}
