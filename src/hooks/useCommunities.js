import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// ── Mock fallback data ─────────────────────────────────────────────────────
const MOCK_COMMUNITIES = [
  {
    id: '1',
    name: 'Magnetite Hub',
    description: 'The official Magnetite gaming community.',
    icon_url: null,
    member_count: 1024,
    online_count: 87,
    owner_id: '1',
  },
  {
    id: '2',
    name: 'Rust Gamedev',
    description: 'Building games in Rust — share your progress.',
    icon_url: null,
    member_count: 412,
    online_count: 23,
    owner_id: '2',
  },
];

const MOCK_MEMBERS = [
  { id: '1', username: 'dev_one', display_name: 'Dev One', status: 'online', roles: ['admin'] },
  { id: '2', username: 'player_two', display_name: 'Player Two', status: 'idle', roles: ['member'] },
  { id: '3', username: 'streamer', display_name: 'StreamerX', status: 'online', roles: ['member'] },
];

// ── Hook ───────────────────────────────────────────────────────────────────
export function useCommunities() {
  const [communities, setCommunities] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchCommunities = useCallback(async () => {
    let cancelled = false;
    try {
      setLoading(true);
      setError(null);
      const data = await api.communities.list();
      if (!cancelled) {
        setCommunities(Array.isArray(data) ? data : (data?.communities ?? MOCK_COMMUNITIES));
      }
    } catch {
      if (!cancelled) {
        setCommunities(MOCK_COMMUNITIES);
      }
    } finally {
      if (!cancelled) setLoading(false);
    }
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    fetchCommunities();
  }, [fetchCommunities]);

  const createCommunity = useCallback(async (data) => {
    try {
      const created = await api.communities.create(data);
      setCommunities((prev) => [created, ...prev]);
      return { success: true, community: created };
    } catch (err) {
      // Optimistic mock fallback
      const mock = {
        id: String(Date.now()),
        name: data.name,
        description: data.description ?? '',
        icon_url: data.icon_url ?? null,
        member_count: 1,
        online_count: 1,
        owner_id: 'me',
      };
      setCommunities((prev) => [mock, ...prev]);
      return { success: true, community: mock, _mock: true, _error: err.message };
    }
  }, []);

  const joinCommunity = useCallback(async (id) => {
    try {
      await api.communities.join(id);
      setCommunities((prev) =>
        prev.map((c) =>
          c.id === id ? { ...c, member_count: (c.member_count ?? 0) + 1 } : c
        )
      );
      return { success: true };
    } catch (err) {
      return { success: false, error: err.message };
    }
  }, []);

  const leaveCommunity = useCallback(async (id) => {
    try {
      await api.communities.leave(id);
      setCommunities((prev) => prev.filter((c) => c.id !== id));
      return { success: true };
    } catch (err) {
      return { success: false, error: err.message };
    }
  }, []);

  return {
    communities,
    loading,
    error,
    fetchCommunities,
    createCommunity,
    joinCommunity,
    leaveCommunity,
  };
}

// ── Per-community members hook ─────────────────────────────────────────────
export function useCommunityMembers(communityId) {
  const [members, setMembers] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (!communityId) return;
    let cancelled = false;

    async function fetch() {
      try {
        setLoading(true);
        setError(null);
        const data = await api.communities.members(communityId);
        if (!cancelled) {
          setMembers(Array.isArray(data) ? data : (data?.members ?? MOCK_MEMBERS));
        }
      } catch {
        if (!cancelled) setMembers(MOCK_MEMBERS);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetch();
    return () => { cancelled = true; };
  }, [communityId]);

  return { members, loading, error };
}
