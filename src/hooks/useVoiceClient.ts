import { useState, useCallback, useEffect, useRef } from 'react';
import { useVoice } from './useVoice';
import { useCommsSocket } from './useCommsSocket';
import type { WsFrame, VoiceParticipant } from '../types/comms';

/**
 * useVoiceClient — the real voice experience orchestrator for Wave 7.
 *
 * Orchestrates:
 *  - getUserMedia(audio) to capture the local microphone
 *  - RTCPeerConnection mesh via useCommsSocket's initPeer / destroyPeer / sendVoiceSignal
 *  - Web Audio AnalyserNode on local + remote streams to detect speaking activity
 *  - Mute (disableTrack) and deafen (silence remote audio elements)
 *  - Remote audio element attachment so participants are heard
 *  - Graceful permission-denied state
 *  - Reduced-motion safe (animation flags only; CSS guards the motion)
 *
 * Designed to be composed *inside* VoicePanel — it owns NO global state.
 */

export interface UseVoiceClientOptions {
  /** for useVoice room listing */
  communityId?: string | null | undefined;
  /** called with server voice_state_update msgs */
  onVoiceStateChange?: (msg: WsFrame) => void;
}

export type PermissionState = 'idle' | 'granted' | 'denied' | 'unavailable';

interface RemoteAudioEntry {
  audioEl: HTMLAudioElement;
  stream: MediaStream;
  stopDetector: () => void;
}

const SPEAKING_THRESHOLD = 18;   // RMS dB above floor that counts as "speaking"
const POLL_INTERVAL_MS   = 80;   // analyse speaking at ~12 fps

// ── Web Audio speaking detector ────────────────────────────────────────────
function createSpeakingDetector(stream: MediaStream | null, onSpeaking: (isSpeaking: boolean) => void): () => void {
  if (!stream || typeof AudioContext === 'undefined') {
    return () => {};
  }

  let ctx: AudioContext;
  let analyser: AnalyserNode;
  let source: MediaStreamAudioSourceNode;
  let rafId: number;
  let stopped = false;

  try {
    ctx      = new AudioContext();
    analyser = ctx.createAnalyser();
    analyser.fftSize = 256;
    analyser.smoothingTimeConstant = 0.6;
    source   = ctx.createMediaStreamSource(stream);
    source.connect(analyser);
  } catch {
    // AudioContext not available (e.g. SSR / blocked)
    return () => {};
  }

  const dataArray = new Uint8Array(analyser.fftSize);
  let lastTick = 0;

  function tick(now: number) {
    if (stopped) return;
    rafId = requestAnimationFrame(tick);

    if (now - lastTick < POLL_INTERVAL_MS) return;
    lastTick = now;

    analyser.getByteTimeDomainData(dataArray);
    // Compute RMS
    let sum = 0;
    for (let i = 0; i < dataArray.length; i++) {
      const v = (dataArray[i] / 128) - 1;
      sum += v * v;
    }
    const rms = Math.sqrt(sum / dataArray.length);
    const db  = 20 * Math.log10(Math.max(rms, 1e-10));
    onSpeaking(db > -SPEAKING_THRESHOLD);
  }

  rafId = requestAnimationFrame(tick);

  return () => {
    stopped = true;
    if (rafId) cancelAnimationFrame(rafId);
    try { source.disconnect(); } catch { /* ignore */ }
    // ctx.close() returns a Promise (rejects if the context is already
    // closing/closed) — a sync try/catch around it does NOT catch that
    // rejection, so it was silently becoming an unhandled promise
    // rejection instead of being ignored as intended. .catch() actually
    // suppresses it.
    ctx.close().catch(() => { /* ignore: already closing/closed */ });
  };
}

// ── Hook ───────────────────────────────────────────────────────────────────
export function useVoiceClient({ communityId = null, onVoiceStateChange }: UseVoiceClientOptions = {}) {
  // ── Underlying data-layer hooks (frozen, do NOT edit) ─────────────────
  const voice = useVoice(communityId);

  // Keep latest callbacks/voice methods in refs so the WS handler is stable
  const voiceRef = useRef(voice);
  const onVoiceStateChangeRef = useRef(onVoiceStateChange);
  useEffect(() => { voiceRef.current = voice; }, [voice]);
  useEffect(() => { onVoiceStateChangeRef.current = onVoiceStateChange; }, [onVoiceStateChange]);

  // Stable voice-state handler — never re-creates so useCommsSocket options
  // don't trigger a new WS subscription on every render.
  const handleVoiceState = useCallback((msg: WsFrame) => {
    if (onVoiceStateChangeRef.current) onVoiceStateChangeRef.current(msg);
    const v = voiceRef.current;
    if (msg.action === 'join')        v.addParticipant(msg.participant as VoiceParticipant);
    else if (msg.action === 'leave')  v.removeParticipant(String(msg.user_id));
    else if (msg.action === 'update') v.updateParticipant(String(msg.user_id), msg.updates as Partial<VoiceParticipant>);
  }, []);

  // We use useCommsSocket for signaling only — no chat handlers needed here
  const socket = useCommsSocket({
    onVoiceState: handleVoiceState,
  });

  // ── Local connection / permission state ───────────────────────────────
  const [permissionState, setPermissionState] = useState<PermissionState>('idle');
  const [localSpeaking, setLocalSpeaking]     = useState(false);

  // ── Remote audio tracking: userId → { audioEl, stream, stopDetector } ──
  const remoteAudioRef = useRef<Record<string, RemoteAudioEntry>>({});
  // ── Local stream analyser cleanup ──────────────────────────────────────
  const localDetectorCleanupRef = useRef<(() => void) | null>(null);
  // ── Local stream ref (for mute track enable/disable) ───────────────────
  const localStreamRef = useRef<MediaStream | null>(null);

  // ── Attach / detach a remote audio element for a stream ──────────────
  const attachRemoteStream = useCallback((userId: string, stream: MediaStream) => {
    // Detach any existing element first
    const existing = remoteAudioRef.current[userId];
    if (existing) {
      existing.stopDetector?.();
      try { existing.audioEl.srcObject = null; } catch { /* ignore */ }
    }

    const audioEl = new Audio();
    audioEl.autoplay    = true;
    audioEl.muted       = voice.deafened; // respect deafen at attach time
    audioEl.srcObject   = stream;

    const stopDetector = createSpeakingDetector(stream, (isSpeaking) => {
      voice.updateParticipant(userId, { speaking: isSpeaking });
    });

    remoteAudioRef.current[userId] = { audioEl, stream, stopDetector };
  }, [voice]);

  const detachRemoteStream = useCallback((userId: string) => {
    const entry = remoteAudioRef.current[userId];
    if (!entry) return;
    entry.stopDetector?.();
    try { entry.audioEl.srcObject = null; } catch { /* ignore */ }
    try { entry.audioEl.pause();           } catch { /* ignore */ }
    delete remoteAudioRef.current[userId];
  }, []);

  // ── Deafen effect: mute / unmute all remote audio elements ────────────
  useEffect(() => {
    Object.values(remoteAudioRef.current).forEach(({ audioEl }) => {
      audioEl.muted = voice.deafened;
    });
  }, [voice.deafened]);

  // ── Mute effect: enable / disable local audio tracks ──────────────────
  useEffect(() => {
    const stream = localStreamRef.current;
    if (!stream) return;
    stream.getAudioTracks().forEach((track) => {
      track.enabled = !voice.muted;
    });
  }, [voice.muted]);

  // ── Join a voice room ─────────────────────────────────────────────────
  const joinVoiceRoom = useCallback(async (roomId: string) => {
    // Leave any current room first (clean slate)
    if (voice.currentRoom) {
      socket.destroyPeer();
      voice.leaveRoom();
      // Detach all remote streams
      Object.keys(remoteAudioRef.current).forEach(detachRemoteStream);
      if (localDetectorCleanupRef.current) {
        localDetectorCleanupRef.current();
        localDetectorCleanupRef.current = null;
      }
      localStreamRef.current = null;
    }

    const joinResult = await voice.joinRoom(roomId);
    if (!joinResult.success) {
      return joinResult;
    }

    const token = joinResult.token as string;

    try {
      const { localStream } = await socket.initPeer(roomId, token, {
        audio: true,
        video: false,
        onTrack: (remoteStream) => {
          // We can't know which userId produced this track here — we attach it
          // keyed by a transient stream id until a voice_state_update arrives.
          attachRemoteStream(remoteStream.id, remoteStream);
        },
        onStateChange: (_state) => {
          // peerState is already tracked inside useCommsSocket
        },
      });

      // Mic permission feedback
      if (localStream) {
        setPermissionState('granted');
        localStreamRef.current = localStream;

        // Apply current mute state immediately to new stream
        localStream.getAudioTracks().forEach((track) => {
          track.enabled = !voice.muted;
        });

        // Local speaking detector
        if (localDetectorCleanupRef.current) localDetectorCleanupRef.current();
        localDetectorCleanupRef.current = createSpeakingDetector(localStream, setLocalSpeaking);
      } else {
        // getUserMedia returned null — permission denied or device unavailable
        setPermissionState('denied');
      }
    } catch (err) {
      const name = (err as { name?: string } | null)?.name;
      if (name === 'NotAllowedError' || name === 'PermissionDeniedError') {
        setPermissionState('denied');
      } else if (name === 'NotFoundError' || name === 'DevicesNotFoundError') {
        setPermissionState('unavailable');
      } else {
        setPermissionState('denied');
      }
    }

    return joinResult;
  }, [voice, socket, attachRemoteStream, detachRemoteStream]);

  // ── Leave the current voice room ──────────────────────────────────────
  const leaveVoiceRoom = useCallback(() => {
    // Signal leave over voice WS
    if (voice.currentRoom) {
      socket.sendVoiceSignal({ type: 'voice_leave', room_id: voice.currentRoom.id });
    }

    socket.destroyPeer();
    voice.leaveRoom();

    // Tear down all remote audio
    Object.keys(remoteAudioRef.current).forEach(detachRemoteStream);

    // Tear down local detector
    if (localDetectorCleanupRef.current) {
      localDetectorCleanupRef.current();
      localDetectorCleanupRef.current = null;
    }
    localStreamRef.current = null;
    setLocalSpeaking(false);
    setPermissionState('idle');
  }, [voice, socket, detachRemoteStream]);

  // ── Cleanup on unmount ────────────────────────────────────────────────
  useEffect(() => {
    // Capture refs at effect-setup time so cleanup reads consistent values.
    const remoteAudio  = remoteAudioRef;
    const localCleanup = localDetectorCleanupRef;
    return () => {
      socket.destroyPeer();
      Object.keys(remoteAudio.current).forEach((uid) => {
        const entry = remoteAudio.current[uid];
        entry?.stopDetector?.();
        try { if (entry?.audioEl) entry.audioEl.srcObject = null; } catch { /* ignore */ }
      });
      if (localCleanup.current) {
        localCleanup.current();
      }
    };
    // Intentional: this effect runs only on mount/unmount. socket.destroyPeer
    // is stable (useCallback with empty deps inside useCommsSocket).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Return shape ──────────────────────────────────────────────────────
  return {
    // Room / participant data (from useVoice)
    rooms:        voice.rooms,
    currentRoom:  voice.currentRoom,
    participants: voice.participants,
    voiceLoading: voice.loading,
    voiceError:   voice.error,
    fetchRooms:   voice.fetchRooms,

    // Controls (from useVoice)
    muted:        voice.muted,
    deafened:     voice.deafened,
    toggleMute:   voice.toggleMute,
    toggleDeafen: voice.toggleDeafen,

    // Client-side speaking detection
    localSpeaking,

    // Connection state (from useCommsSocket)
    peerState:      socket.peerState,
    voiceConnected: socket.voiceConnected,

    // Permission / device state
    permissionState, // 'idle'|'granted'|'denied'|'unavailable'

    // Actions
    joinVoiceRoom,
    leaveVoiceRoom,
  };
}
