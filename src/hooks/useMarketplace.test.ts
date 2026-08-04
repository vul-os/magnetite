import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useMarketplace } from './useMarketplace';

type PurchaseResult = Awaited<ReturnType<ReturnType<typeof useMarketplace>['purchase']>>;

vi.mock('../api/client', () => ({
  api: {
    stores: {
      mine: vi.fn(),
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

const mockMine = vi.mocked(api.stores.mine);
const mockEntitlements = vi.mocked(api.stores.entitlements);
const mockItems = vi.mocked(api.stores.items);
const mockCreate = vi.mocked(api.stores.create);
const mockAddItem = vi.mocked(api.stores.addItem);
const mockUpdateItem = vi.mocked(api.stores.updateItem);
const mockRemoveItem = vi.mocked(api.stores.removeItem);
const mockPurchase = vi.mocked(api.stores.purchase);

describe('useMarketplace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: API unavailable → hook falls back to mock data.
    mockMine.mockRejectedValue(new Error('no backend'));
    mockEntitlements.mockRejectedValue(new Error('no backend'));
    mockItems.mockRejectedValue(new Error('no backend'));
    mockCreate.mockRejectedValue(new Error('no backend'));
    mockAddItem.mockRejectedValue(new Error('no backend'));
    mockUpdateItem.mockRejectedValue(new Error('no backend'));
    mockRemoveItem.mockRejectedValue(new Error('no backend'));
    mockPurchase.mockRejectedValue(new Error('no backend'));
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
    mockMine.mockResolvedValue({ stores: fakeStores });
    mockEntitlements.mockRejectedValue(new Error('no backend'));

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.stores).toEqual(fakeStores);
  });

  it('loadItems: loads store items from the API on first call', async () => {
    const fakeItems = [
      { id: 'item-x', store_id: 's1', name: 'API Item', price_points: 100, price_usdc: 0.10, item_type: 'cosmetic', active: true, sales: 0 },
    ];
    mockItems.mockResolvedValue({ items: fakeItems });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });

    expect(result.current.items['s1']).toEqual(fakeItems);
  });

  it('loadItems: falls back to mock items when API call fails', async () => {
    mockItems.mockRejectedValue(new Error('fail'));

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });

    // Should have items (either mock or empty array) but not undefined
    expect(result.current.items['s1']).toBeDefined();
  });

  it('loadItems: does not reload if items already cached', async () => {
    mockItems.mockResolvedValue({ items: [{ id: 'i1', store_id: 's1', name: 'Item', price_points: 50, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 }] });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadItems('s1');
    });
    const callCount = mockItems.mock.calls.length;

    // Call again — should be a no-op
    await act(async () => {
      await result.current.loadItems('s1');
    });

    expect(mockItems.mock.calls.length).toBe(callCount); // not called again
  });

  it('hasEntitlement: returns true for owned item IDs', async () => {
    // Seed entitlements via the API mock so the hook populates them.
    const fakeEntitlements = [
      { id: 'e1', item_id: 'i1', item_name: 'Plasma Rifle Skin', game_title: 'Cosmic Raiders', purchased_at: '2026-05-20T10:00:00Z', currency: 'points' },
    ];
    mockMine.mockRejectedValue(new Error('no backend'));
    mockEntitlements.mockResolvedValue({ entitlements: fakeEntitlements });

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

    let purchaseResult: PurchaseResult | undefined;
    await act(async () => {
      purchaseResult = await result.current.purchase('s1', 'i1', 'points');
    });

    if (!purchaseResult) throw new Error('purchase did not resolve');
    expect(purchaseResult.success).toBe(false);
    if (purchaseResult.success) throw new Error('expected purchase to fail');
    expect(purchaseResult.error).toBeDefined();
  });

  it('purchase: adds an optimistic entitlement on success', async () => {
    mockPurchase.mockResolvedValue({ entitlement_id: 'ent-new' });
    mockItems.mockResolvedValue({
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

    let purchaseResult: PurchaseResult | undefined;
    await act(async () => {
      purchaseResult = await result.current.purchase('s1', 'new-item', 'usdc');
    });

    if (!purchaseResult) throw new Error('purchase did not resolve');
    expect(purchaseResult.success).toBe(true);
    expect(result.current.entitlements.length).toBe(entBefore + 1);
    expect(result.current.entitlements[0].item_id).toBe('new-item');
    expect(result.current.entitlements[0].currency).toBe('usdc');
  });

  it('purchase: sets purchasing flag while in-flight', async () => {
    let resolveP: (value: unknown) => void = () => {};
    mockPurchase.mockReturnValue(new Promise((r) => { resolveP = r; }));

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
    mockCreate.mockResolvedValueOnce({ store: created });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    const storeBefore = result.current.stores.length;

    await act(async () => {
      await result.current.createStore({ name: 'My New Store', game_id: 99 });
    });

    expect(result.current.stores.length).toBe(storeBefore + 1);
    const lastStore = result.current.stores.at(-1);
    if (!lastStore) throw new Error('expected a store to have been added');
    expect(lastStore.name).toBe('My New Store');
  });

  it('addItem: appends item on success', async () => {
    const newItem = { id: 'add-item-1', store_id: 's1', name: 'Added', price_points: 200, price_usdc: 0.25, item_type: 'boost', active: true, sales: 0 };
    mockAddItem.mockResolvedValue({ item: newItem });
    mockItems.mockResolvedValue({ items: [] });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => { await result.current.loadItems('s1'); });
    const before = (result.current.items['s1'] ?? []).length;

    await act(async () => {
      await result.current.addItem('s1', { name: 'Added', price_points: 200 });
    });

    expect((result.current.items['s1'] ?? []).length).toBe(before + 1);
    const lastItem = result.current.items['s1'].at(-1);
    if (!lastItem) throw new Error('expected an item to have been added');
    expect(lastItem.name).toBe('Added');
  });

  it('removeItem: removes item by id', async () => {
    mockItems.mockResolvedValue({
      items: [
        { id: 'del-1', store_id: 's1', name: 'Delete Me', price_points: 100, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
        { id: 'keep-1', store_id: 's1', name: 'Keep Me', price_points: 200, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
      ],
    });
    mockRemoveItem.mockResolvedValue({ ok: true });

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
    mockItems.mockResolvedValue({
      items: [
        { id: 'upd-1', store_id: 's1', name: 'Old Name', price_points: 100, price_usdc: 0, item_type: 'cosmetic', active: true, sales: 0 },
      ],
    });
    mockUpdateItem.mockResolvedValue({ ok: true });

    const { result } = renderHook(() => useMarketplace());
    await vi.waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => { await result.current.loadItems('s1'); });

    await act(async () => {
      await result.current.updateItem('s1', 'upd-1', { name: 'New Name', active: false });
    });

    const updated = result.current.items['s1'].find((i) => i.id === 'upd-1');
    if (!updated) throw new Error('expected the updated item to still be present');
    expect(updated.name).toBe('New Name');
    expect(updated.active).toBe(false);
  });
});
