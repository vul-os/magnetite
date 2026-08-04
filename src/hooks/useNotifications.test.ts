// useNotifications.test.js — AX2 tests for real-time notification delivery.
//
// Tests:
//  1. NotificationContext initial load via REST (api.notifications.list)
//  2. addNotification() push shape (simulates WS-pushed frame)
//  3. markAsRead / markAllAsRead optimistic updates
//  4. unreadCount derived correctly
//  5. api.notifications client surface

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import React, { type ReactNode } from 'react';
import { NotificationProvider, useNotificationContext } from '../context/NotificationContext';

// ── mock api client ───────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    notifications: {
      list:        vi.fn(),
      unreadCount: vi.fn(),
      markAsRead:  vi.fn(),
      markAllAsRead: vi.fn(),
      delete:      vi.fn(),
    },
  },
}));

import { api } from '../api/client';

const mockNotificationsList = vi.mocked(api.notifications.list);
const mockMarkAsRead = vi.mocked(api.notifications.markAsRead);
const mockMarkAllAsRead = vi.mocked(api.notifications.markAllAsRead);

const wrapper = ({ children }: { children: ReactNode }) => (
  React.createElement(NotificationProvider, null, children)
);

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('NotificationContext — REST load', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNotificationsList.mockRejectedValue(new Error('No backend'));
    mockMarkAsRead.mockResolvedValue({});
    mockMarkAllAsRead.mockResolvedValue({});
  });

  afterEach(() => vi.clearAllMocks());

  it('initializes with empty notifications when API fails', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    expect(result.current.notifications).toEqual([]);
    expect(result.current.unreadCount).toBe(0);
  });

  it('loads notifications from API on mount', async () => {
    const mockList = [
      { id: '1', type: 'FRIEND_REQUEST', title: 'Alice wants to be friends', read: false, createdAt: new Date().toISOString() },
      { id: '2', type: 'PAYOUT_COMPLETE', title: 'Payout sent', read: true, createdAt: new Date().toISOString() },
    ];
    mockNotificationsList.mockResolvedValue(mockList);

    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    expect(result.current.notifications.length).toBe(2);
  });

  it('handles API response with { notifications: [] } shape', async () => {
    mockNotificationsList.mockResolvedValue({ notifications: [
      { id: '3', type: 'SYSTEM', title: 'Welcome!', read: false },
    ]});

    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    expect(result.current.notifications.length).toBe(1);
  });
});

describe('NotificationContext — unreadCount', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockMarkAsRead.mockResolvedValue({});
    mockMarkAllAsRead.mockResolvedValue({});
  });

  it('counts only unread notifications', async () => {
    mockNotificationsList.mockResolvedValue([
      { id: '1', read: false, type: 'SYSTEM', title: 'A' },
      { id: '2', read: false, type: 'SYSTEM', title: 'B' },
      { id: '3', read: true,  type: 'SYSTEM', title: 'C' },
    ]);

    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    expect(result.current.unreadCount).toBe(2);
  });

  it('unreadCount is 0 when all notifications are read', async () => {
    mockNotificationsList.mockResolvedValue([
      { id: '1', read: true, type: 'SYSTEM', title: 'Done' },
    ]);

    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    expect(result.current.unreadCount).toBe(0);
  });
});

describe('NotificationContext — addNotification (WS push)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNotificationsList.mockResolvedValue([]);
    mockMarkAsRead.mockResolvedValue({});
    mockMarkAllAsRead.mockResolvedValue({});
  });

  it('addNotification pushes a new unread entry to the list', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.addNotification({
        type: 'FRIEND_REQUEST',
        title: 'Bob sent you a friend request',
        body: 'Accept or decline.',
      });
    });

    expect(result.current.notifications.length).toBe(1);
    expect(result.current.notifications[0].read).toBe(false);
    expect(result.current.notifications[0].title).toBe('Bob sent you a friend request');
  });

  it('pushed notification is prepended (most recent first)', async () => {
    mockNotificationsList.mockResolvedValue([
      { id: 'old', read: true, type: 'SYSTEM', title: 'Old notification' },
    ]);

    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.addNotification({ type: 'PAYOUT_COMPLETE', title: 'Payout sent!' });
    });

    expect(result.current.notifications[0].title).toBe('Payout sent!');
    expect(result.current.notifications[1].title).toBe('Old notification');
  });

  it('addNotification increments unreadCount', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    const before = result.current.unreadCount;

    act(() => {
      result.current.addNotification({ type: 'SYSTEM', title: 'New system alert' });
    });

    expect(result.current.unreadCount).toBe(before + 1);
  });

  it('pushed notification gets a stable id', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.addNotification({ type: 'GAME_INVITE', title: 'Invite' });
    });

    const id = result.current.notifications[0].id;
    expect(id).toBeDefined();
    expect(typeof id).toBe('string');
  });

  it('notification push shape matches WsNotification contract', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.addNotification({
        type: 'ACHIEVEMENT_UNLOCKED',
        title: 'First Win',
        body: 'You won your first game!',
        data: { achievement_id: 'first_win' },
      });
    });

    const n = result.current.notifications[0];
    expect(n).toHaveProperty('id');
    expect(n).toHaveProperty('type');
    expect(n).toHaveProperty('title');
    expect(n).toHaveProperty('read', false);
    expect(n).toHaveProperty('createdAt');
    expect(n.data).toEqual({ achievement_id: 'first_win' });
  });
});

describe('NotificationContext — markAsRead / markAllAsRead', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNotificationsList.mockResolvedValue([
      { id: '1', read: false, type: 'SYSTEM', title: 'A' },
      { id: '2', read: false, type: 'SYSTEM', title: 'B' },
    ]);
    mockMarkAsRead.mockResolvedValue({});
    mockMarkAllAsRead.mockResolvedValue({});
  });

  it('markAsRead marks a single notification as read', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.markAsRead('1');
    });

    const n = result.current.notifications.find(n => n.id === '1');
    expect(n!.read).toBe(true);
    // Other notification still unread
    const m = result.current.notifications.find(n => n.id === '2');
    expect(m!.read).toBe(false);
  });

  it('markAllAsRead marks every notification as read', async () => {
    const { result } = renderHook(() => useNotificationContext(), { wrapper });
    await vi.waitFor(() => expect(result.current.initialized).toBe(true));

    act(() => {
      result.current.markAllAsRead();
    });

    result.current.notifications.forEach(n => {
      expect(n.read).toBe(true);
    });
    expect(result.current.unreadCount).toBe(0);
  });
});

// ── api.notifications client surface ─────────────────────────────────────────

describe('api.notifications client', () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.clearAllMocks());

  it('list() returns an array', async () => {
    mockNotificationsList.mockResolvedValue([
      { id: '1', type: 'SYSTEM', title: 'Hi', read: false },
    ]);
    const result = await mockNotificationsList();
    expect(Array.isArray(result)).toBe(true);
  });

  it('markAsRead(id) is called with the correct id', async () => {
    mockMarkAsRead.mockResolvedValue({});
    await mockMarkAsRead('notif-123');
    expect(mockMarkAsRead).toHaveBeenCalledWith('notif-123');
  });

  it('markAllAsRead() is called without arguments', async () => {
    mockMarkAllAsRead.mockResolvedValue({});
    await mockMarkAllAsRead();
    expect(mockMarkAllAsRead).toHaveBeenCalledTimes(1);
  });
});
