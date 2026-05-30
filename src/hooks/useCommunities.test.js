import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useCommunities, useCommunityMembers } from './useCommunities';

vi.mock('../api/client', () => ({
  api: {
    communities: {
      list: vi.fn(),
      create: vi.fn(),
      join: vi.fn(),
      leave: vi.fn(),
      members: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

describe('useCommunities', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: API unavailable → mock fallback.
    api.communities.list.mockRejectedValue(new Error('no backend'));
    api.communities.create.mockRejectedValue(new Error('no backend'));
    api.communities.join.mockRejectedValue(new Error('no backend'));
    api.communities.leave.mockRejectedValue(new Error('no backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('starts loading then populates with mock data on API failure', async () => {
    const { result } = renderHook(() => useCommunities());

    expect(result.current.loading).toBe(true);

    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.communities.length).toBeGreaterThan(0);
    expect(result.current.error).toBeNull();
  });

  it('uses API data when backend returns valid communities', async () => {
    const fakeCommunities = [
      { id: 'api-1', name: 'API Community', member_count: 10, online_count: 2 },
    ];
    api.communities.list.mockResolvedValue({ communities: fakeCommunities });

    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.communities).toEqual(fakeCommunities);
  });

  it('uses API data when backend returns a plain array', async () => {
    const fakeCommunities = [
      { id: 'arr-1', name: 'Array Community', member_count: 5, online_count: 1 },
    ];
    api.communities.list.mockResolvedValue(fakeCommunities);

    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.communities).toEqual(fakeCommunities);
  });

  it('exposes fetchCommunities for manual refresh', async () => {
    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(typeof result.current.fetchCommunities).toBe('function');

    // Resolve a fresh list on the second call
    const freshList = [{ id: 'new-1', name: 'Fresh Community', member_count: 1, online_count: 0 }];
    api.communities.list.mockResolvedValueOnce(freshList);

    await act(async () => {
      await result.current.fetchCommunities();
    });

    expect(result.current.communities).toEqual(freshList);
  });

  it('createCommunity: adds community to state on API success', async () => {
    const created = { id: 'c-new', name: 'Brand New', description: '', icon_url: null, member_count: 1, online_count: 1 };
    api.communities.create.mockResolvedValue(created);

    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const before = result.current.communities.length;
    let res;
    await act(async () => {
      res = await result.current.createCommunity({ name: 'Brand New' });
    });

    expect(res.success).toBe(true);
    expect(result.current.communities.length).toBe(before + 1);
    expect(result.current.communities[0].id).toBe('c-new');
  });

  it('createCommunity: falls back to optimistic mock on API failure', async () => {
    // api.communities.create already mocked to reject
    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const before = result.current.communities.length;
    let res;
    await act(async () => {
      res = await result.current.createCommunity({ name: 'Offline Community' });
    });

    expect(res.success).toBe(true);
    expect(res._mock).toBe(true);
    expect(result.current.communities.length).toBe(before + 1);
    expect(result.current.communities[0].name).toBe('Offline Community');
  });

  it('joinCommunity: increments member_count on success', async () => {
    api.communities.join.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const targetId = result.current.communities[0].id;
    const before = result.current.communities[0].member_count;

    await act(async () => {
      await result.current.joinCommunity(targetId);
    });

    const after = result.current.communities.find((c) => c.id === targetId);
    expect(after.member_count).toBe(before + 1);
  });

  it('joinCommunity: returns success:false on API failure', async () => {
    // api.communities.join mocked to reject
    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let res;
    await act(async () => {
      res = await result.current.joinCommunity('some-id');
    });

    expect(res.success).toBe(false);
    expect(res.error).toBeDefined();
  });

  it('leaveCommunity: removes community from state on success', async () => {
    api.communities.leave.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const targetId = result.current.communities[0].id;
    const before = result.current.communities.length;

    await act(async () => {
      await result.current.leaveCommunity(targetId);
    });

    expect(result.current.communities.length).toBe(before - 1);
    expect(result.current.communities.find((c) => c.id === targetId)).toBeUndefined();
  });

  it('leaveCommunity: returns success:false on API failure', async () => {
    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let res;
    await act(async () => {
      res = await result.current.leaveCommunity('unknown-id');
    });

    expect(res.success).toBe(false);
  });

  it('communities have the expected shape', async () => {
    const { result } = renderHook(() => useCommunities());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    result.current.communities.forEach((c) => {
      expect(c).toHaveProperty('id');
      expect(c).toHaveProperty('name');
      expect(c).toHaveProperty('member_count');
    });
  });
});

describe('useCommunityMembers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.communities.members = vi.fn().mockRejectedValue(new Error('no backend'));
  });

  it('returns empty members when communityId is null', async () => {
    const { result } = renderHook(() => useCommunityMembers(null));
    // Should not start loading with no communityId
    expect(result.current.members).toEqual([]);
  });

  it('populates members with mock data on API failure', async () => {
    const { result } = renderHook(() => useCommunityMembers('comm-1'));

    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.members.length).toBeGreaterThan(0);
  });

  it('uses API members when backend returns valid data', async () => {
    const fakeMembers = [
      { id: 'm1', username: 'user_a', display_name: 'User A', status: 'online', roles: ['member'] },
    ];
    api.communities.members.mockResolvedValue({ members: fakeMembers });

    const { result } = renderHook(() => useCommunityMembers('comm-1'));
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.members).toEqual(fakeMembers);
  });

  it('handles communityId change by re-fetching', async () => {
    api.communities.members.mockResolvedValue({ members: [{ id: 'm1', username: 'a', status: 'online', roles: [] }] });

    const { result, rerender } = renderHook(
      ({ id }) => useCommunityMembers(id),
      { initialProps: { id: 'comm-1' } }
    );

    await vi.waitFor(() => expect(result.current.loading).toBe(false));
    const firstCallCount = api.communities.members.mock.calls.length;

    // Switch to a different community
    api.communities.members.mockResolvedValue({ members: [{ id: 'm2', username: 'b', status: 'idle', roles: [] }] });
    rerender({ id: 'comm-2' });

    await vi.waitFor(() => expect(result.current.loading).toBe(false));
    expect(api.communities.members.mock.calls.length).toBeGreaterThan(firstCallCount);
  });
});
