import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// ── Mock fallback data — only used when VITE_USE_MOCKS=true ───────────────
const MOCK_CHANNELS = import.meta.env.VITE_USE_MOCKS === 'true'
  ? [
      { id: 'c1', name: 'general',        kind: 'text',  community_id: '1', position: 0 },
      { id: 'c2', name: 'announcements',  kind: 'text',  community_id: '1', position: 1 },
      { id: 'c3', name: 'game-dev',       kind: 'text',  community_id: '1', position: 2 },
      { id: 'c4', name: 'General Voice',  kind: 'voice', community_id: '1', position: 3 },
      { id: 'c5', name: 'Game Room',      kind: 'voice', community_id: '1', position: 4 },
    ]
  : null;

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// ── Hook ───────────────────────────────────────────────────────────────────
export function useChannels(communityId) {
  const [channels, setChannels] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (!communityId) {
      setChannels([]);
      return;
    }
    let cancelled = false;

    async function fetchChannels() {
      try {
        setLoading(true);
        setError(null);

        if (USE_MOCKS) {
          if (!cancelled) setChannels(MOCK_CHANNELS ?? []);
          return;
        }

        const data = await api.channels.list(communityId);
        if (!cancelled) {
          const list = Array.isArray(data) ? data : (data?.channels ?? null);
          setChannels(list ?? []);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err.message ?? 'Failed to load channels');
          setChannels([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchChannels();
    return () => { cancelled = true; };
  }, [communityId]);

  const createChannel = useCallback(async (data) => {
    if (!communityId) return { success: false, error: 'No community selected' };
    try {
      const created = await api.channels.create(communityId, data);
      setChannels((prev) => [...prev, created]);
      return { success: true, channel: created };
    } catch (err) {
      if (USE_MOCKS) {
        // In mock mode, optimistic local add is acceptable
        const mock = {
          id: String(Date.now()),
          name: data.name,
          kind: data.kind ?? 'text',
          community_id: communityId,
          position: channels.length,
        };
        setChannels((prev) => [...prev, mock]);
        return { success: true, channel: mock, _mock: true, _error: err.message };
      }
      // Real mode: surface the error
      return { success: false, error: err.message };
    }
  }, [communityId, channels.length]);

  // Convenience selectors
  const textChannels = channels.filter((c) => c.kind === 'text');
  const voiceChannels = channels.filter((c) => c.kind === 'voice');

  return { channels, textChannels, voiceChannels, loading, error, createChannel };
}
