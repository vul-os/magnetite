import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────
//
// Money model (§3.6 PaymentRail): non-custodial. Every `*_usdc` field is a plain
// USDC amount (6-dp stablecoin units rendered as decimals). Purchases move funds
// buyer wallet → developer wallet atomically and emit a **signed receipt**;
// entitlements are receipt-backed, so each one carries the receipt that proves it.

export interface Store {
  id: string;
  name: string;
  game_id: number | string;
  game_title?: string;
  description?: string;
  item_count?: number;
  revenue_usdc?: number;
  revenue_points?: number;
  [key: string]: unknown;
}

export interface MarketplaceItem {
  id: string;
  store_id: string;
  name: string;
  description?: string;
  price_points?: number;
  price_usdc?: number;
  item_type?: string;
  active?: boolean;
  sales?: number;
  [key: string]: unknown;
}

export interface Entitlement {
  id: string;
  item_id: string;
  item_name: string;
  game_title?: string;
  purchased_at: string;
  currency: string;
  receipt_id: string | null;
  rail_pubkey: string | null;
  total: number;
  protocol_fee: number;
  [key: string]: unknown;
}

const MOCK_STORES: Store[] = [
  {
    id: 's1', name: 'Neon Forge Shop', game_id: 1, game_title: 'Cosmic Raiders',
    description: 'Official cosmetics for Cosmic Raiders.',
    item_count: 12, revenue_usdc: 4_820.5, revenue_points: 98_000,
  },
  {
    id: 's2', name: 'Speed Goods', game_id: 3, game_title: 'Speed Legends',
    description: 'Liveries, horns and trail effects.',
    item_count: 7, revenue_usdc: 1_240.0, revenue_points: 31_500,
  },
];

const MOCK_ITEMS: Record<string, MarketplaceItem[]> = {
  s1: [
    { id: 'i1', store_id: 's1', name: 'Plasma Rifle Skin',   description: 'Electric-teal finish.',   price_points: 800,  price_usdc: 0.99, item_type: 'cosmetic', active: true,  sales: 182 },
    { id: 'i2', store_id: 's1', name: 'Void Shield Pack',    description: 'Three animated shields.', price_points: 1500, price_usdc: 1.99, item_type: 'bundle',   active: true,  sales: 74  },
    { id: 'i3', store_id: 's1', name: 'XP Accelerator (7d)', description: '1.5× XP for 7 days.',     price_points: 500,  price_usdc: 0.49, item_type: 'boost',    active: false, sales: 420 },
  ],
  s2: [
    { id: 'i4', store_id: 's2', name: 'Carbon Livery',       description: 'Matte-black racing skin.', price_points: 1200, price_usdc: 1.49, item_type: 'cosmetic', active: true, sales: 98  },
    { id: 'i5', store_id: 's2', name: 'Neon Trail Effect',   description: 'Glowing exhaust effect.',  price_points: 600,  price_usdc: 0.79, item_type: 'cosmetic', active: true, sales: 211 },
  ],
};

/**
 * Receipt-backed entitlements. `receipt_id` + `rail_pubkey` are the proof the node
 * verifies before granting the item; `total` / `protocol_fee` are USDC amounts
 * (protocol fee defaults to 0 bps — the developer receives the full subtotal).
 */
const MOCK_ENTITLEMENTS: Entitlement[] = [
  {
    id: 'e1', item_id: 'i1', item_name: 'Plasma Rifle Skin', game_title: 'Cosmic Raiders',
    purchased_at: '2026-05-20T10:00:00Z', currency: 'points',
    receipt_id: 'rcpt_01HZK8Q3M2Y7N4RB',
    rail_pubkey: 'ed25519:9f3c7a1e4b82d05c',
    total: 0, protocol_fee: 0,
  },
  {
    id: 'e2', item_id: 'i4', item_name: 'Carbon Livery', game_title: 'Speed Legends',
    purchased_at: '2026-05-18T14:22:00Z', currency: 'usdc',
    receipt_id: 'rcpt_01HZJ2W9F5X1C8TD',
    rail_pubkey: 'ed25519:2b6d90fe1a47c3b8',
    total: 1.49, protocol_fee: 0,
  },
  {
    id: 'e3', item_id: 'i2', item_name: 'Void Shield Pack', game_title: 'Cosmic Raiders',
    purchased_at: '2026-05-11T09:05:00Z', currency: 'usdc',
    receipt_id: 'rcpt_01HZ9M4T6K0P2VQA',
    rail_pubkey: 'ed25519:7c15be03d9a64f2e',
    total: 1.99, protocol_fee: 0,
  },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

// ─────────────────────────────────────────────────────────────────────────────

export function useMarketplace() {
  const [stores, setStores]             = useState<Store[]>(USE_MOCKS ? MOCK_STORES : []);
  const [items, setItems]               = useState<Record<string, MarketplaceItem[]>>({});
  const [entitlements, setEntitlements] = useState<Entitlement[]>(USE_MOCKS ? MOCK_ENTITLEMENTS : []);
  const [loading, setLoading]           = useState(!USE_MOCKS);
  const [error, setError]               = useState<string | null>(null);
  const [purchasing, setPurchasing]     = useState(false);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const [storesRes, entRes] = await Promise.allSettled([
          api.stores.mine(),
          api.stores.entitlements(),
        ]);

        if (!cancelled) {
          if (storesRes.status === 'fulfilled' && Array.isArray((storesRes.value as { stores?: unknown })?.stores)) {
            setStores((storesRes.value as { stores: Store[] }).stores);
          } else if (storesRes.status === 'fulfilled' && Array.isArray(storesRes.value)) {
            setStores(storesRes.value as Store[]);
          } else if (storesRes.status === 'rejected') {
            setError(errMessage(storesRes.reason, 'Failed to load stores'));
          }

          if (entRes.status === 'fulfilled' && Array.isArray((entRes.value as { entitlements?: unknown })?.entitlements)) {
            setEntitlements((entRes.value as { entitlements: Entitlement[] }).entitlements);
          } else if (entRes.status === 'fulfilled' && Array.isArray(entRes.value)) {
            setEntitlements(entRes.value as Entitlement[]);
          }
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void load();
    return () => { cancelled = true; };
  }, []);

  /** Lazy-load items for a store. Falls back to mock only when VITE_USE_MOCKS=true. */
  const loadItems = useCallback(async (storeId: string) => {
    if (items[storeId]) return; // already loaded
    try {
      const data = await api.stores.items(storeId);
      const body = data as { items?: MarketplaceItem[] } | MarketplaceItem[] | null;
      const list = Array.isArray((body as { items?: unknown })?.items)
        ? (body as { items: MarketplaceItem[] }).items
        : (Array.isArray(body) ? body : null);
      if (USE_MOCKS) {
        setItems(prev => ({ ...prev, [storeId]: list ?? MOCK_ITEMS[storeId] ?? [] }));
      } else {
        setItems(prev => ({ ...prev, [storeId]: list ?? [] }));
      }
    } catch {
      if (USE_MOCKS) {
        setItems(prev => ({ ...prev, [storeId]: MOCK_ITEMS[storeId] ?? [] }));
      } else {
        setItems(prev => ({ ...prev, [storeId]: [] }));
      }
    }
  }, [items]);

  /**
   * Create a store for a game. A store belongs to a game on this backend, so
   * the game id goes in the path: POST /marketplace/games/:game_id/store.
   */
  const createStore = useCallback(async ({ game_id: gameId, ...data }: { game_id: string | number; [key: string]: unknown }) => {
    if (!gameId) throw new Error('A game must be selected before creating a store.');
    const result = await api.stores.create(gameId, data);
    const newStore = (result as { store?: Store; data?: Store } | null)?.store ?? (result as { store?: Store; data?: Store } | null)?.data ?? null;
    if (!newStore) throw new Error('The node did not return the created store.');
    setStores(s => [...s, newStore]);
    return newStore;
  }, []);

  const addItem = useCallback(async (storeId: string, data: Partial<MarketplaceItem>) => {
    const result = await api.stores.addItem(storeId, data);
    const newItem = (result as { item?: MarketplaceItem } | null)?.item
      ?? ({ id: `i${Date.now()}`, store_id: storeId, active: true, sales: 0, ...data } as MarketplaceItem);
    setItems(prev => ({ ...prev, [storeId]: [...(prev[storeId] ?? []), newItem] }));
    return newItem;
  }, []);

  const updateItem = useCallback(async (storeId: string, itemId: string, data: Partial<MarketplaceItem>) => {
    await api.stores.updateItem(storeId, itemId, data);
    setItems(prev => ({
      ...prev,
      [storeId]: (prev[storeId] ?? []).map(i => i.id === itemId ? { ...i, ...data } : i),
    }));
  }, []);

  /**
   * UNAVAILABLE — the backend mounts no delete-item route, so an item cannot be
   * removed from a store on this node. Consumers should read
   * `capabilities.removeItem` and not render the control at all rather than
   * offer a button that 404s. Kept callable so the shape is stable.
   */
  const removeItem = useCallback(async (storeId: string, itemId: string) => {
    await api.stores.removeItem(storeId, itemId);
    setItems(prev => ({
      ...prev,
      [storeId]: (prev[storeId] ?? []).filter(i => i.id !== itemId),
    }));
  }, []);

  /**
   * What this node can actually do, verified against the mounted routes in
   * backend/src/api/marketplace.rs. `false` means no route exists — a UI must
   * say so plainly instead of rendering a control that silently fails.
   */
  const capabilities = {
    listAllStores: false, // no public GET /marketplace/stores; only /my-stores
    createStore:   true,
    deleteStore:   false, // no DELETE /marketplace/stores/:id
    addItem:       true,
    updateItem:    true,
    removeItem:    false, // no DELETE route for items
    purchase:      true,
  };

  /**
   * Wallet checkout. Funds move buyer wallet → developer wallet atomically and the
   * rail returns a signed receipt; the entitlement we record is backed by it.
   * `currency` is 'points' (off-chain, not money) or 'usdc'.
   */
  const purchase = useCallback(async (storeId: string, itemId: string, currency = 'points') => {
    setPurchasing(true);
    try {
      const result = await api.stores.purchase(storeId, itemId, { currency });
      const receipt = (result as { receipt?: Partial<Entitlement> } | null)?.receipt ?? (result as Partial<Entitlement> | null) ?? {};
      // Optimistic, receipt-backed entitlement
      const storeItems = items[storeId] ?? [];
      const item = storeItems.find(i => i.id === itemId);
      if (item) {
        setEntitlements(e => [
          {
            id: `e${Date.now()}`,
            item_id: itemId,
            item_name: item.name,
            purchased_at: new Date().toISOString(),
            currency,
            receipt_id:  receipt.receipt_id  ?? (receipt as { id?: string }).id ?? null,
            rail_pubkey: receipt.rail_pubkey ?? null,
            total:        receipt.total        ?? (currency === 'usdc' ? (item.price_usdc ?? 0) : 0),
            protocol_fee: receipt.protocol_fee ?? 0,
          },
          ...e,
        ]);
      }
      return { success: true, result };
    } catch (err) {
      return { success: false, error: errMessage(err, 'unknown error') };
    } finally {
      setPurchasing(false);
    }
  }, [items]);

  const hasEntitlement = useCallback((itemId: string) => {
    return entitlements.some(e => e.item_id === itemId);
  }, [entitlements]);

  return {
    stores, items, entitlements, loading, error, purchasing, capabilities,
    loadItems, createStore, addItem, updateItem, removeItem, purchase, hasEntitlement,
  };
}
