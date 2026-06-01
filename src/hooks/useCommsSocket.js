import { useState, useEffect, useCallback, useRef } from 'react';
import { useWebSocket } from './useWebSocket';

/**
 * useCommsSocket — the realtime backbone for Wave 6 comms.
 *
 * Architecture (per DECISIONS.md §4b):
 *  - Text chat + presence: Axum WebSocket at /ws/comms
 *  - Voice signaling: WebRTC SDP/ICE relay over /ws/voice?token=<jwt>&room=<room_token>
 *    Mesh for small rooms; SFU (LiveKit/mediasoup) is the scale path.
 *
 * Message envelope: { type, ...payload }
 *
 * Inbound types handled (snake_case, matching backend rename_all="snake_case"):
 *   message_created    — new chat message in a channel
 *   presence_update    — user came online/offline/idle
 *   voice_state_update — participant joined/left/muted in a voice room
 *   offer              — WebRTC SDP offer relayed from a peer
 *   answer             — WebRTC SDP answer relayed from a peer
 *   ice_candidate      — ICE candidate relayed from a peer
 *   typing_notify      — typing indicator from backend
 *   typing_start/stop  — typing indicator (outbound only)
 *   room_state         — full room snapshot on voice join
 *   participant_joined — a peer just joined the voice room
 *   participant_left   — a peer left the voice room
 *   dm_message         — direct message
 *
 * Outbound helpers exposed: sendChatMessage, sendDM, sendTypingStart,
 *   sendTypingStop, sendVoiceSignal (SDP/ICE relay)
 *
 * WebRTC peer helper (minimal mesh for small rooms):
 *   initPeer(roomId, roomToken, { onTrack, onStateChange })
 *   → starts getUserMedia, creates RTCPeerConnection, relays SDP/ICE via WS
 *   destroyPeer()
 */

const WS_COMMS_PATH = '/ws/comms';

const ICE_SERVERS = [{ urls: 'stun:stun.l.google.com:19302' }];

// ── helpers ────────────────────────────────────────────────────────────────
function noop() {}

// Build the voice WS URL including the mandatory ?room= param.
// useWebSocket will add ?token=<jwt> on top.
function buildVoiceUrl(roomToken) {
  return `/ws/voice?room=${encodeURIComponent(roomToken)}`;
}

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

  // ── Voice signaling WS — lazy: only open when we have a room token ────────
  // voiceWsUrl is null until initPeer is called; null disables the WS.
  const [voiceWsUrl, setVoiceWsUrl] = useState(null);

  const {
    isConnected: voiceConnected,
    lastMessage: voiceLastMessage,
    sendMessage: sendVoiceSignal,
  } = useWebSocket(voiceWsUrl ?? WS_COMMS_PATH, {  // dummy path when null so hook doesn't crash
    autoReconnect: false,
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

  // ── Track remote peers so we can target offer/answer/ICE correctly ────────
  // Populated from room_state and participant_joined server frames.
  const remotePeersRef = useRef([]); // Array<string> of user_id strings

  // ── Inbound: chat/presence WS messages ───────────────────────────────────
  useEffect(() => {
    if (!lastMessage) return;
    const msg = lastMessage;

    switch (msg.type) {
      // Backend ServerFrame serialises MessageCreated as 'message_created'
      case 'message_created':
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

      // Backend ServerFrame serialises TypingNotify as 'typing_notify'
      case 'typing_notify': {
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

      // Keep handling typing_start from other transports for compat
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
    if (!voiceLastMessage) return;
    if (voiceWsUrl === null) return; // voice WS not yet active
    const msg = voiceLastMessage;

    // Track remote peers from room_state snapshot (sent by backend on join)
    if (msg.type === 'room_state' && Array.isArray(msg.participants)) {
      remotePeersRef.current = msg.participants.map((p) =>
        typeof p === 'object' ? String(p.user_id) : String(p)
      );
    }

    if (msg.type === 'participant_joined') {
      const uid = String(msg.user_id);
      if (!remotePeersRef.current.includes(uid)) {
        remotePeersRef.current = [...remotePeersRef.current, uid];
      }
    }

    if (msg.type === 'participant_left') {
      remotePeersRef.current = remotePeersRef.current.filter(
        (id) => id !== String(msg.user_id)
      );
    }

    if (!pcRef.current) return;

    (async () => {
      const pc = pcRef.current;
      if (!pc) return;
      try {
        // Backend ServerVoiceFrame serialises Offer as 'offer', Answer as 'answer',
        // IceCandidate as 'ice_candidate' — all with from_user_id rather than room_id.
        if (msg.type === 'offer') {
          await pc.setRemoteDescription(new RTCSessionDescription({ type: 'offer', sdp: msg.sdp }));
          const answer = await pc.createAnswer();
          await pc.setLocalDescription(answer);
          // Reply to the specific peer that sent the offer
          if (msg.from_user_id) {
            sendVoiceSignal({ type: 'answer', to_user_id: msg.from_user_id, sdp: answer.sdp });
          }
        } else if (msg.type === 'answer') {
          await pc.setRemoteDescription(new RTCSessionDescription({ type: 'answer', sdp: msg.sdp }));
        } else if (msg.type === 'ice_candidate') {
          const candidate = new RTCIceCandidate({
            candidate: msg.candidate,
            sdpMid: msg.sdp_mid ?? null,
            sdpMLineIndex: msg.sdp_mline_index ?? null,
          });
          await pc.addIceCandidate(candidate);
        }
      } catch {
        // Silently swallow — peer may have left
      }
    })();
  }, [voiceLastMessage, voiceWsUrl, sendVoiceSignal]);

  // Notify backend which channel we're listening to when it changes.
  // Backend ClientFrame::JoinChannel serialises as 'join_channel'.
  useEffect(() => {
    if (!isConnected || !channelId) return;
    sendMessage({ type: 'join_channel', channel_id: channelId, community_id: communityId });
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
      // Backend ClientFrame::SendMessage serialises as 'send_message'
      sendMessage({
        type: 'send_message',
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
    // Backend ClientFrame::TypingStart serialises as 'typing_start'
    sendMessage({ type: 'typing_start', channel_id: channelId });
  }, [sendMessage, channelId]);

  const sendTypingStop = useCallback(() => {
    if (!channelId) return;
    // Backend ClientFrame::TypingStop serialises as 'typing_stop'
    sendMessage({ type: 'typing_stop', channel_id: channelId });
  }, [sendMessage, channelId]);

  // ── WebRTC peer helper ────────────────────────────────────────────────────
  /**
   * Initialise a WebRTC peer connection for a voice room.
   *
   * @param {string} roomId      - voice room id (UUID)
   * @param {string} roomToken   - join token from api.voice.joinToken() — used as ?room= in WS URL
   * @param {object} opts
   *   onTrack(stream)        - called when a remote track is received
   *   onStateChange(state)   - called when connection state changes
   *   audio {boolean}        - request mic (default true)
   *   video {boolean}        - request camera (default false)
   */
  const initPeer = useCallback(
    async (roomId, roomToken, { onTrack = noop, onStateChange = noop, audio = true, video = false } = {}) => {
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

      // Open the voice WS with the mandatory ?room= param.
      // useWebSocket will append ?token=<jwt> automatically.
      setVoiceWsUrl(buildVoiceUrl(roomToken));

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

      // ICE candidate → relay over voice WS.
      // Backend ClientVoiceFrame::IceCandidate expects { to_user_id, candidate, sdp_mid, sdp_mline_index }.
      pc.onicecandidate = (event) => {
        if (event.candidate) {
          const json = event.candidate.toJSON();
          // Send to all known remote peers
          remotePeersRef.current.forEach((peerId) => {
            sendVoiceSignal({
              type: 'ice_candidate',
              to_user_id: peerId,
              candidate: json.candidate,
              sdp_mid: json.sdpMid ?? null,
              sdp_mline_index: json.sdpMLineIndex ?? null,
            });
          });
        }
      };

      // Connection state
      pc.onconnectionstatechange = () => {
        const s = pc.connectionState;
        setPeerState(s);
        onStateChange(s);
      };

      // Send join intent over voice WS.
      // Backend ClientVoiceFrame::JoinRoom expects { room_token }, serialised as 'join_room'.
      sendVoiceSignal({ type: 'join_room', room_token: roomToken });

      // As offerer: create offer (sent to new peers arriving AFTER us).
      // Backend ClientVoiceFrame::Offer expects { to_user_id, sdp }, serialised as 'offer'.
      try {
        const offer = await pc.createOffer({ offerToReceiveAudio: true, offerToReceiveVideo: video });
        await pc.setLocalDescription(offer);
        // Send offer to all currently-known peers
        remotePeersRef.current.forEach((peerId) => {
          sendVoiceSignal({ type: 'offer', to_user_id: peerId, sdp: offer.sdp });
        });
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
    remotePeersRef.current = [];
    setPeerState('idle');
    setVoiceWsUrl(null);
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
