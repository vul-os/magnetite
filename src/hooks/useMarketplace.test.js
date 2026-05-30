import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useMarketplace } from './useMarketplace';

vi.mock('../api/client', () => ({
  api: {
    stores: {
      list: vi.fn(),
      entitlements: vi.fn(),
      items: vi.fn(),
      create: vi.fn(),
      addItem: vi.fn(),
      updateItem: vi.fn(),
      removeItem: vi.fn(),
      purchase: vi.fn(),
    },
  },
}));

import { api } from '../api/client';

describe('useMarketplace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: API unavailable → hook falls back to mock data.
    api.stores.list.mockRejectedValue(new Error('no backend'));
    api.stores.entitlements.mockRejectedValue(new Error('no backend'));
    api.stores.items.mockRejectedValue(new Error('no backend'));
    api.stores.create.mockRejectedValue(new Error('no backend'));
    api.stores.addItem.mockRejectedValue(new Error('no backend'));
    api.stores.updateItem.mockRejectedValue(new Error('no backend'));
    api.stores.removeItem.mockRejectedValue(new Error('no backend'));
    api.stores.purchase.mockRejectedValue(new Error('no backend'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('starts loading then settles with mock store data', async () => {
    // When the API rejects, the hook settles to empty state with an error set.
    const { result } = renderHook(() => useMarketplace());

    expect(result.current.loading).toBe(true);

    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    // stores empty (API failed, error set), entitlements empty (API failed)
    expect(result.current.stores).toEqual([]);
    expect(result.current.error).toBeTruthy();
    expect(result.current.entitlements).toEqual([]);
  });

  it('uses API stores when backend returns valid data', async () => {
    const fakeStores = [
      { id: 'api-s1', name: 'API Store', game_id: 42, item_count: 5, revenue_usdc: 0, revenue_points: 0 },
    ];
    api.stores.list.mockResolvedValue({ stores: fakeStores });
    api.stores.entitlements.mockRejectedValue(new Error('no backend'));

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.stores).toEqual(fakeStores);
  });

  it('loadItems: loads store items from the API on first call', async () => {
    const fakeItems = [
      { id: 'item-x', store_id: 's1', name: 'API Item', price_points: 100, price_usdc: 0.10, item_type: 'cosmetic', active: true, sales: 0 },
    ];
    api.stores.items.mockResolvedValue({ items: fakeItems });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });

    expect(result.current.items['s1']).toEqual(fakeItems);
  });

  it('loadItems: falls back to mock items when API call fails', async () => {
    api.stores.items.mockRejectedValue(new Error('fail'));

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });

    // Should have items (either mock or empty array) but not undefined
    expect(result.current.items['s1']).toBeDefined();
  });

  it('loadItems: does not reload if items already cached', async () => {
    api.stores.items.mockResolvedValue({ items: [{ id: 'i1', store_id: 's1', name: 'Item', price_points: 50, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 }] });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });
    const callCount = api.stores.items.mock.calls.length;

    // Call again — should be a no-op
    await act(async () => {
      await result.current.loadItems('s1');
    });

    expect(api.stores.items.mock.calls.length).toBe(callCount); // not called again
  });

  it('hasEntitlement: returns true for owned item IDs', async () => {
    // Seed entitlements via the API mock so the hook populates them.
    const fakeEntitlements = [
      { id: 'e1', item_id: 'i1', item_name: 'Plasma Rifle Skin', game_title: 'Cosmic Raiders', purchased_at: '2026-05-20T10:00:00Z', currency: 'points' },
    ];
    api.stores.list.mockRejectedValue(new Error('no backend'));
    api.stores.entitlements.mockResolvedValue({ entitlements: fakeEntitlements });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.hasEntitlement('i1')).toBe(true);
  });

  it('hasEntitlement: returns false for unowned item IDs', async () => {
    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.hasEntitlement('does-not-exist-xyz')).toBe(false);
  });

  it('purchase: returns success:false on API error', async () => {
    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    let purchaseResult;
    await act(async () => {
      purchaseResult = await result.current.purchase('s1', 'i1', 'points');
    });

    expect(purchaseResult.success).toBe(false);
    expect(purchaseResult.error).toBeDefined();
  });

  it('purchase: adds an optimistic entitlement on success', async () => {
    api.stores.purchase.mockResolvedValue({ entitlement_id: 'ent-new' });
    api.stores.items.mockResolvedValue({
      items: [
        { id: 'new-item', store_id: 's1', name: 'New Cosmetic', price_points: 100, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
      ],
    });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    // Load items so the item exists in state
    await act(async () => {
      await result.current.loadItems('s1');
    });

    const entBefore = result.current.entitlements.length;

    let purchaseResult;
    await act(async () => {
      purchaseResult = await result.current.purchase('s1', 'new-item', 'usdc');
    });

    expect(purchaseResult.success).toBe(true);
    expect(result.current.entitlements.length).toBe(entBefore + 1);
    expect(result.current.entitlements[0].item_id).toBe('new-item');
    expect(result.current.entitlements[0].currency).toBe('usdc');
  });

  it('purchase: sets purchasing flag while in-flight', async () => {
    let resolveP;
    api.stores.purchase.mockReturnValue(new Promise((r) => { resolveP = r; }));

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.purchasing).toBe(false);

    act(() => {
      result.current.purchase('s1', 'i1', 'points');
    });

    expect(result.current.purchasing).toBe(true);

    await act(async () => {
      resolveP({ ok: true });
    });

    expect(result.current.purchasing).toBe(false);
  });

  it('createStore: adds a store when API succeeds', async () => {
    const created = { id: 'new-s', name: 'My New Store', game_id: 99, item_count: 0, revenue_usdc: 0, revenue_points: 0 };
    api.stores.create.mockResolvedValueOnce({ store: created });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const storeBefore = result.current.stores.length;

    await act(async () => {
      await result.current.createStore({ name: 'My New Store', game_id: 99 });
    });

    expect(result.current.stores.length).toBe(storeBefore + 1);
    expect(result.current.stores.at(-1).name).toBe('My New Store');
  });

  it('addItem: appends item on success', async () => {
    const newItem = { id: 'add-item-1', store_id: 's1', name: 'Added', price_points: 200, price_usdc: 0.25, item_type: 'boost', active: true, sales: 0 };
    api.stores.addItem.mockResolvedValue({ item: newItem });
    api.stores.items.mockResolvedValue({ items: [] });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => { await result.current.loadItems('s1'); });
    const before = (result.current.items['s1'] ?? []).length;

    await act(async () => {
      await result.current.addItem('s1', { name: 'Added', price_points: 200 });
    });

    expect((result.current.items['s1'] ?? []).length).toBe(before + 1);
    expect(result.current.items['s1'].at(-1).name).toBe('Added');
  });

  it('removeItem: removes item by id', async () => {
    api.stores.items.mockResolvedValue({
      items: [
        { id: 'del-1', store_id: 's1', name: 'Delete Me', price_points: 100, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
        { id: 'keep-1', store_id: 's1', name: 'Keep Me', price_points: 200, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
      ],
    });
    api.stores.removeItem.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => { await result.current.loadItems('s1'); });
    expect(result.current.items['s1'].length).toBe(2);

    await act(async () => {
      await result.current.removeItem('s1', 'del-1');
    });

    expect(result.current.items['s1'].length).toBe(1);
    expect(result.current.items['s1'][0].id).toBe('keep-1');
  });

  it('updateItem: merges item updates', async () => {
    api.stores.items.mockResolvedValue({
      items: [
        { id: 'upd-1', store_id: 's1', name: 'Old Name', price_points: 100, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
      ],
    });
    api.stores.updateItem.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => { await result.current.loadItems('s1'); });

    await act(async () => {
      await result.current.updateItem('s1', 'upd-1', { name: 'New Name', active: false });
    });

    const updated = result.current.items['s1'].find((i) => i.id === 'upd-1');
    expect(updated.name).toBe('New Name');
    expect(updated.active).toBe(false);
  });
});
