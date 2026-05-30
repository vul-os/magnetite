import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api/client';

/**
 * useVoice — manages voice room listing, joining (token fetch), and the
 * local mic/speaker state that the UI reflects. The actual WebRTC peer
 * connections are managed inside useCommsSocket's peer helper. This hook
 * is the "intent" layer; useCommsSocket is the "transport" layer.
 */

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────
const MOCK_ROOMS = [
  { id: 'vr1', name: 'General Voice', community_id: '1', participant_count: 3, max_participants: 20 },
  { id: 'vr2', name: 'Game Room Alpha', community_id: '1', participant_count: 0, max_participants: 8 },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export function useVoice(communityId) {
  const [rooms, setRooms] = useState(USE_MOCKS ? MOCK_ROOMS : []);
  const [currentRoom, setCurrentRoom] = useState(null);
  const [joinToken, setJoinToken] = useState(null);
  const [muted, setMuted] = useState(false);
  const [deafened, setDeafened] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  // Ref so callbacks don't go stale (updated in effect, not during render)
  const currentRoomRef = useRef(null);
  useEffect(() => {
    currentRoomRef.current = currentRoom;
  }, [currentRoom]);

  // ── Room list ─────────────────────────────────────────────────────────────
  const fetchRooms = useCallback(async () => {
    if (!communityId) return;
    if (USE_MOCKS) return;

    let cancelled = false;
    try {
      setLoading(true);
      setError(null);
      const data = await api.voice.rooms(communityId);
      if (!cancelled) {
        setRooms(Array.isArray(data) ? data : (data?.rooms ?? []));
      }
    } catch (err) {
      if (!cancelled) {
        setError(err.message ?? 'Failed to load voice rooms');
        setRooms([]);
      }
    } finally {
      if (!cancelled) setLoading(false);
    }
    return () => { cancelled = true; };
  }, [communityId]);

  // ── Join a voice room ─────────────────────────────────────────────────────
  /**
   * Fetches a signaling join token for the given roomId.
   * The token is passed to useCommsSocket's peer helper to start the
   * WebRTC negotiation (SDP offer/answer relayed via the signaling WS).
   */
  const joinRoom = useCallback(async (roomId) => {
    if (currentRoomRef.current?.id === roomId) {
      return { success: true, already: true };
    }
    try {
      setLoading(true);
      setError(null);

      let tokenData;
      if (USE_MOCKS) {
        tokenData = { token: `mock-voice-token-${roomId}-${Date.now()}`, room_id: roomId };
      } else {
        tokenData = await api.voice.joinToken(roomId);
      }

      const room = rooms.find((r) => r.id === roomId) ?? { id: roomId, name: 'Voice Room' };
      setCurrentRoom(room);
      setJoinToken(tokenData.token);
      return { success: true, token: tokenData.token, room };
    } catch (err) {
      setError(err.message);
      return { success: false, error: err.message };
    } finally {
      setLoading(false);
    }
  }, [rooms]);

  // ── Leave the current voice room ──────────────────────────────────────────
  const leaveRoom = useCallback(() => {
    setCurrentRoom(null);
    setJoinToken(null);
    setMuted(false);
    setDeafened(false);
  }, []);

  // ── Mic / speaker controls ────────────────────────────────────────────────
  const toggleMute = useCallback(() => setMuted((m) => !m), []);
  const toggleDeafen = useCallback(() => {
    setDeafened((d) => {
      if (!d) setMuted(true); // deafen implies mute
      return !d;
    });
  }, []);

  // ── Participant state (updated by useCommsSocket voice_state events) ──────
  const [participants, setParticipants] = useState([]);

  const addParticipant = useCallback((participant) => {
    setParticipants((prev) => {
      if (prev.find((p) => p.id === participant.id)) return prev;
      return [...prev, participant];
    });
  }, []);

  const removeParticipant = useCallback((userId) => {
    setParticipants((prev) => prev.filter((p) => p.id !== userId));
  }, []);

  const updateParticipant = useCallback((userId, updates) => {
    setParticipants((prev) =>
      prev.map((p) => (p.id === userId ? { ...p, ...updates } : p))
    );
  }, []);

  return {
    rooms,
    currentRoom,
    joinToken,
    muted,
    deafened,
    loading,
    error,
    participants,
    fetchRooms,
    joinRoom,
    leaveRoom,
    toggleMute,
    toggleDeafen,
    addParticipant,
    removeParticipant,
    updateParticipant,
  };
}
