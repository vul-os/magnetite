import { useState, useEffect, useCallback, useRef } from 'react';

/**
 * Manages user presence state for a set of user IDs.
 *
 * In production, presence updates arrive over the comms WebSocket (see
 * useCommsSocket). This hook provides the local state store + helpers that
 * useCommsSocket (or a component) can push updates into via `setPresence`.
 *
 * Presence statuses: 'online' | 'idle' | 'dnd' | 'offline'
 *
 * Mock data is seeded only when VITE_USE_MOCKS=true; otherwise the map
 * starts empty and is populated exclusively by real WS presence_update events.
 */

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────
const MOCK_PRESENCE = {
  '1': { status: 'online', activity: 'Playing Rust Racer', updated_at: Date.now() },
  '2': { status: 'idle', activity: null, updated_at: Date.now() - 600_000 },
  '3': { status: 'online', activity: 'Streaming', updated_at: Date.now() - 30_000 },
};

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// Stable status ordering for sorting member lists
const STATUS_ORDER = { online: 0, dnd: 1, idle: 2, offline: 3 };

export function usePresence(userIds = []) {
  // presenceMap: { [userId]: { status, activity, updated_at } }
  const [presenceMap, setPresenceMap] = useState(USE_MOCKS ? MOCK_PRESENCE : {});
  const idSetRef = useRef(new Set());

  // Keep idSet in sync
  useEffect(() => {
    idSetRef.current = new Set(userIds);
  }, [userIds]);

  // Real presence updates arrive from the comms WebSocket via setPresence / bulkSetPresence.
  // No API polling here — the WS push model is the authoritative source.

  /** Called by useCommsSocket when a presence_update WS message arrives. */
  const setPresence = useCallback((userId, presenceData) => {
    setPresenceMap((prev) => ({
      ...prev,
      [userId]: { ...presenceData, updated_at: Date.now() },
    }));
  }, []);

  /** Batch update from a presence snapshot (e.g. on channel join). */
  const bulkSetPresence = useCallback((snapshot) => {
    setPresenceMap((prev) => ({ ...prev, ...snapshot }));
  }, []);

  /** Get the presence entry for a single user. */
  const getPresence = useCallback(
    (userId) => presenceMap[userId] ?? { status: 'offline', activity: null },
    [presenceMap]
  );

  /** Sort a member list by presence status then name. */
  const sortByPresence = useCallback(
    (members) =>
      [...members].sort((a, b) => {
        const sa = STATUS_ORDER[presenceMap[a.id]?.status ?? 'offline'] ?? 3;
        const sb = STATUS_ORDER[presenceMap[b.id]?.status ?? 'offline'] ?? 3;
        if (sa !== sb) return sa - sb;
        return (a.display_name ?? a.username ?? '').localeCompare(
          b.display_name ?? b.username ?? ''
        );
      }),
    [presenceMap]
  );

  const onlineCount = Object.values(presenceMap).filter(
    (p) => p.status === 'online' || p.status === 'dnd'
  ).length;

  return { presenceMap, getPresence, setPresence, bulkSetPresence, sortByPresence, onlineCount };
}
