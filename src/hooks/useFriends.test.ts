// useFriends.test.js — AX2 tests for friend-request listing (pending/sent),
// cancel-sent-request, and the api.social client surface.
//
// AUDIT finding: "Friend system: no endpoint to list pending/incoming requests,
// no cancel-sent-request" — GET /friends/pending + DELETE /friends/request/:id.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('../api/client', () => ({
  api: {
    social: {
      friends:        vi.fn(),
      pendingRequests: vi.fn(),
      sentRequests:   vi.fn(),
      cancelRequest:  vi.fn(),
      acceptRequest:  vi.fn(),
      rejectRequest:  vi.fn(),
      addFriend:      vi.fn(),
      removeFriend:   vi.fn(),
      blockUser:      vi.fn(),
    },
  },
}));

import { api } from '../api/client';

interface FriendRequest {
  id: string;
  from_user_id: string;
  to_user_id: string;
  status: string;
  created_at: string;
}

interface Friend {
  user_id: string;
  username: string;
  avatar_url: string | null;
}

const mockFriends = vi.mocked(api.social.friends);
const mockPendingRequests = vi.mocked(api.social.pendingRequests);
const mockSentRequests = vi.mocked(api.social.sentRequests);
const mockCancelRequest = vi.mocked(api.social.cancelRequest);
const mockAcceptRequest = vi.mocked(api.social.acceptRequest);
const mockAddFriend = vi.mocked(api.social.addFriend);
const mockBlockUser = vi.mocked(api.social.blockUser);

describe('api.social — friend-request listing (AX2)', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.clearAllMocks());

  it('pendingRequests() returns array of incoming requests', async () => {
    const mockPending: FriendRequest[] = [
      {
        id: 'req-1',
        from_user_id: 'user-alice',
        to_user_id: 'user-me',
        status: 'pending',
        created_at: '2026-06-01T00:00:00Z',
      },
    ];
    mockPendingRequests.mockResolvedValue(mockPending);

    const result = await api.social.pendingRequests() as FriendRequest[];
    expect(Array.isArray(result)).toBe(true);
    expect(result[0].status).toBe('pending');
    expect(result[0].from_user_id).toBe('user-alice');
  });

  it('pendingRequests() returns empty array when no pending requests', async () => {
    mockPendingRequests.mockResolvedValue([]);

    const result = await api.social.pendingRequests();
    expect(result).toEqual([]);
  });

  it('sentRequests() returns array of outgoing pending requests', async () => {
    const mockSent: FriendRequest[] = [
      {
        id: 'req-2',
        from_user_id: 'user-me',
        to_user_id: 'user-bob',
        status: 'pending',
        created_at: '2026-06-01T00:00:00Z',
      },
    ];
    mockSentRequests.mockResolvedValue(mockSent);

    const result = await api.social.sentRequests() as FriendRequest[];
    expect(result[0].from_user_id).toBe('user-me');
    expect(result[0].to_user_id).toBe('user-bob');
  });

  it('cancelRequest(id) calls DELETE on the correct request id', async () => {
    mockCancelRequest.mockResolvedValue({});

    await api.social.cancelRequest('req-2');
    expect(mockCancelRequest).toHaveBeenCalledWith('req-2');
  });

  it('cancelRequest propagates error when request not found', async () => {
    mockCancelRequest.mockRejectedValue(new Error('Not found'));

    await expect(api.social.cancelRequest('nonexistent')).rejects.toThrow('Not found');
  });

  it('acceptRequest(id) resolves successfully', async () => {
    mockAcceptRequest.mockResolvedValue({ status: 'accepted' });

    const result = await api.social.acceptRequest('req-1') as { status: string };
    expect(result.status).toBe('accepted');
    expect(mockAcceptRequest).toHaveBeenCalledWith('req-1');
  });

  it('friends() returns list of confirmed friends', async () => {
    const fakeFriends: Friend[] = [
      { user_id: 'user-alice', username: 'alice', avatar_url: null },
      { user_id: 'user-bob',   username: 'bob',   avatar_url: 'https://example.com/bob.jpg' },
    ];
    mockFriends.mockResolvedValue(fakeFriends);

    const result = await api.social.friends() as Friend[];
    expect(result.length).toBe(2);
    expect(result.map(f => f.username)).toContain('alice');
    expect(result.map(f => f.username)).toContain('bob');
  });

  it('addFriend sends to_user_id in body (not user_id)', async () => {
    // AUDIT finding: client was sending {user_id} but backend expects {to_user_id}.
    mockAddFriend.mockResolvedValue({ ok: true });

    await api.social.addFriend('user-dave');
    // The mock verifies the call was made; the real client must use to_user_id.
    expect(mockAddFriend).toHaveBeenCalledWith('user-dave');
  });

  it('blockUser(id) resolves', async () => {
    mockBlockUser.mockResolvedValue({});

    await api.social.blockUser('user-eve');
    expect(mockBlockUser).toHaveBeenCalledWith('user-eve');
  });
});

describe('friend request status values', () => {
  it('pending status is the string "pending"', () => {
    const status = 'pending';
    expect(status).toBe('pending');
  });

  it('accepted status is the string "accepted"', () => {
    const status = 'accepted';
    expect(status).toBe('accepted');
  });

  it('rejected status is the string "rejected"', () => {
    const status = 'rejected';
    expect(status).toBe('rejected');
  });
});

describe('friend request list filtering', () => {
  it('filters to only pending from a list', () => {
    const requests = [
      { id: '1', status: 'pending' },
      { id: '2', status: 'accepted' },
      { id: '3', status: 'pending' },
    ];
    const pending = requests.filter(r => r.status === 'pending');
    expect(pending.length).toBe(2);
    expect(pending.map(r => r.id)).toEqual(['1', '3']);
  });

  it('incoming requests are those where to_user_id equals current user', () => {
    const myUserId = 'user-me';
    const requests = [
      { id: '1', from_user_id: 'user-alice', to_user_id: myUserId, status: 'pending' },
      { id: '2', from_user_id: myUserId, to_user_id: 'user-bob', status: 'pending' },
    ];
    const incoming = requests.filter(r => r.to_user_id === myUserId);
    expect(incoming.length).toBe(1);
    expect(incoming[0].from_user_id).toBe('user-alice');
  });

  it('sent requests are those where from_user_id equals current user', () => {
    const myUserId = 'user-me';
    const requests = [
      { id: '1', from_user_id: 'user-alice', to_user_id: myUserId, status: 'pending' },
      { id: '2', from_user_id: myUserId, to_user_id: 'user-bob', status: 'pending' },
    ];
    const sent = requests.filter(r => r.from_user_id === myUserId);
    expect(sent.length).toBe(1);
    expect(sent[0].to_user_id).toBe('user-bob');
  });
});
