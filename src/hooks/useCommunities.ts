import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import type { Community, CommunityMember, ActionResult } from '../types/comms';

// ── Mock fallback data — only used when VITE_USE_MOCKS=true ───────────────
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

const MOCK_COMMUNITIES: Community[] | null = USE_MOCKS
  ? [
      { id: '1', name: 'Magnetite Hub',  description: 'The official Magnetite gaming community.', icon_url: null, member_count: 1024, online_count: 87, owner_id: '1' },
      { id: '2', name: 'Rust Gamedev',   description: 'Building games in Rust — share your progress.', icon_url: null, member_count: 412, online_count: 23, owner_id: '2' },
    ]
  : null;

const MOCK_MEMBERS: CommunityMember[] | null = USE_MOCKS
  ? [
      { id: '1', username: 'dev_one',     display_name: 'Dev One',    status: 'online', roles: ['admin']  },
      { id: '2', username: 'player_two',  display_name: 'Player Two', status: 'idle',   roles: ['member'] },
      { id: '3', username: 'streamer',    display_name: 'StreamerX',  status: 'online', roles: ['member'] },
    ]
  : null;

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

// ── Hook ───────────────────────────────────────────────────────────────────
export function useCommunities() {
  const [communities, setCommunities] = useState<Community[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCommunities = useCallback(async () => {
    let cancelled = false;
    try {
      setLoading(true);
      setError(null);

      if (USE_MOCKS) {
        if (!cancelled) setCommunities(MOCK_COMMUNITIES ?? []);
        return;
      }

      const data = await api.communities.list();
      if (!cancelled) {
        const list = Array.isArray(data) ? data : ((data as { communities?: Community[] } | null)?.communities ?? null);
        setCommunities((list as Community[]) ?? []);
      }
    } catch (err) {
      if (!cancelled) {
        setError(errMessage(err, 'Failed to load communities'));
        setCommunities([]);
      }
    } finally {
      if (!cancelled) setLoading(false);
    }
    return () => { cancelled = true; };
  }, []);

  // Initial data fetch on mount; fetchCommunities synchronizes with the
  // communities API (external system) and manages its own loading state.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void fetchCommunities();
  }, [fetchCommunities]);

  const createCommunity = useCallback(async (data: { name: string; description?: string; icon_url?: string | null }): Promise<ActionResult> => {
    try {
      const created = await api.communities.create(data) as Community;
      setCommunities((prev) => [created, ...prev]);
      return { success: true, community: created };
    } catch (err) {
      if (USE_MOCKS) {
        // In mock mode, optimistic local add is acceptable
        const mock: Community = {
          id: String(Date.now()),
          name: data.name,
          description: data.description ?? '',
          icon_url: data.icon_url ?? null,
          member_count: 1,
          online_count: 1,
          owner_id: 'me',
        };
        setCommunities((prev) => [mock, ...prev]);
        return { success: true, community: mock, _mock: true, _error: errMessage(err, 'unknown error') };
      }
      // Real mode: surface the error
      return { success: false, error: errMessage(err, 'unknown error') };
    }
  }, []);

  const joinCommunity = useCallback(async (id: string): Promise<ActionResult> => {
    try {
      await api.communities.join(id);
      setCommunities((prev) =>
        prev.map((c) =>
          c.id === id ? { ...c, member_count: (c.member_count ?? 0) + 1 } : c
        )
      );
      return { success: true };
    } catch (err) {
      return { success: false, error: errMessage(err, 'unknown error') };
    }
  }, []);

  const leaveCommunity = useCallback(async (id: string): Promise<ActionResult> => {
    try {
      await api.communities.leave(id);
      setCommunities((prev) => prev.filter((c) => c.id !== id));
      return { success: true };
    } catch (err) {
      return { success: false, error: errMessage(err, 'unknown error') };
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
export function useCommunityMembers(communityId: string | null) {
  const [members, setMembers] = useState<CommunityMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!communityId) return;
    let cancelled = false;

    async function fetchMembers() {
      try {
        setLoading(true);
        setError(null);

        if (USE_MOCKS) {
          if (!cancelled) setMembers(MOCK_MEMBERS ?? []);
          return;
        }

        const data = await api.communities.members(communityId as string);
        if (!cancelled) {
          const list = Array.isArray(data) ? data : ((data as { members?: CommunityMember[] } | null)?.members ?? null);
          setMembers((list as CommunityMember[]) ?? []);
        }
      } catch (err) {
        if (!cancelled) {
          setError(errMessage(err, 'Failed to load members'));
          setMembers([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void fetchMembers();
    return () => { cancelled = true; };
  }, [communityId]);

  return { members, loading, error };
}
