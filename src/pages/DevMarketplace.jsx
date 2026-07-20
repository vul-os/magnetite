import { useState, useEffect, useMemo } from 'react';
import Layout from '../components/Layout';
import Skeleton from '../components/skeletons/Skeleton';
import EmptyState from '../components/empty/EmptyState';
import { Unavailable } from '../components/state/Unavailable';
import { useMarketplace } from '../hooks/useMarketplace';
import './DevMarketplace.css';

// ── Icons ─────────────────────────────────────────────────────────────────────

function StoreIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z" />
      <line x1="3" x2="21" y1="6" y2="6" />
      <path d="M16 10a4 4 0 0 1-8 0" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

function PencilIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </svg>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const ITEM_TYPES = ['cosmetic', 'boost', 'bundle', 'currency', 'other'];

const EMPTY_STORE_FORM = { name: '', game_id: '', description: '' };
const EMPTY_ITEM_FORM  = {
  name: '', description: '', price_points: '', price_usdc: '', item_type: 'cosmetic',
};

function formatCurrency(n) {
  return `$${Number(n).toFixed(2)}`;
}

// ─────────────────────────────────────────────────────────────────────────────

export default function DevMarketplace() {
  const {
    stores, items, loading,
    loadItems, createStore, addItem, updateItem,
  } = useMarketplace();

  const [selectedStoreId, setSelectedStoreId] = useState(null);

  // Derive activeStore from stores list (no setState in effect needed)
  const activeStore = useMemo(
    () => stores.find(s => s.id === selectedStoreId) ?? stores[0] ?? null,
    [stores, selectedStoreId]
  );

  // Modals
  const [showCreateStore, setShowCreateStore] = useState(false);
  const [showAddItem,     setShowAddItem]     = useState(false);
  const [editingItem,     setEditingItem]     = useState(null); // item obj

  const [storeForm, setStoreForm] = useState(EMPTY_STORE_FORM);
  const [itemForm,  setItemForm]  = useState(EMPTY_ITEM_FORM);
  const [saving,    setSaving]    = useState(false);
  const [msg,       setMsg]       = useState(null);

  // Load items for the active store whenever it changes
  useEffect(() => {
    if (activeStore) {
      loadItems(activeStore.id);
    }
  }, [activeStore, loadItems]);

  function flash(text, type = 'success') {
    setMsg({ text, type });
    setTimeout(() => setMsg(null), 3500);
  }

  async function handleCreateStore(e) {
    e.preventDefault();
    setSaving(true);
    try {
      const store = await createStore(storeForm);
      setSelectedStoreId(store.id);
      setShowCreateStore(false);
      setStoreForm(EMPTY_STORE_FORM);
      flash('Store created!');
    } catch (err) {
      flash(err.message || 'Failed to create store.', 'error');
    } finally {
      setSaving(false);
    }
  }

  async function handleAddItem(e) {
    e.preventDefault();
    if (!activeStore) return;
    setSaving(true);
    try {
      await addItem(activeStore.id, {
        ...itemForm,
        price_points: Number(itemForm.price_points) || 0,
        price_usdc:   Number(itemForm.price_usdc)   || 0,
      });
      setShowAddItem(false);
      setItemForm(EMPTY_ITEM_FORM);
      flash('Item added!');
    } catch (err) {
      flash(err.message || 'Failed to add item.', 'error');
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveItem(e) {
    e.preventDefault();
    if (!activeStore || !editingItem) return;
    setSaving(true);
    try {
      await updateItem(activeStore.id, editingItem.id, {
        ...itemForm,
        price_points: Number(itemForm.price_points) || 0,
        price_usdc:   Number(itemForm.price_usdc)   || 0,
      });
      setEditingItem(null);
      setItemForm(EMPTY_ITEM_FORM);
      flash('Item updated!');
    } catch (err) {
      flash(err.message || 'Failed to update item.', 'error');
    } finally {
      setSaving(false);
    }
  }

  // Removing an item is not implemented on this backend (no delete route), so
  // no delete control is rendered — see the notice above the item table.

  function openEditItem(item) {
    setEditingItem(item);
    setItemForm({
      name:         item.name,
      description:  item.description,
      price_points: String(item.price_points),
      price_usdc:   String(item.price_usdc),
      item_type:    item.item_type,
    });
  }

  function selectStore(store) {
    setSelectedStoreId(store.id);
  }

  const storeItems = activeStore ? (items[activeStore.id] ?? null) : null;
  const isLoadingItems = activeStore && storeItems === null;

  return (
    <Layout>
      <div className="devmp-page reveal">

        {/* ── Header ── */}
        <header className="devmp-header reveal-1">
          <span className="kicker">// Developer Tools</span>
          <h1>Dev Marketplace</h1>
          <p className="devmp-subtitle">Create stores, manage items, and track sales for your games.</p>
        </header>

        {/* ── Flash message ── */}
        {msg && (
          <div className={`devmp-msg devmp-msg-${msg.type}`} role="status" aria-live="polite">
            {msg.text}
          </div>
        )}

        <div className="devmp-layout">

          {/* ── Sidebar: store list ── */}
          <aside className="devmp-sidebar" aria-label="Your stores">
            <div className="devmp-sidebar-header">
              <span className="sidebar-title">Your Stores</span>
              <button
                className="btn btn-primary btn-sm devmp-new-store-btn"
                onClick={() => setShowCreateStore(true)}
                aria-label="Create new store"
              >
                <span className="btn-icon" aria-hidden="true"><PlusIcon /></span>
                New Store
              </button>
            </div>

            {loading ? (
              Array.from({ length: 2 }).map((_, i) => (
                <div key={i} className="devmp-store-item-skeleton">
                  <Skeleton variant="text" width="80%" height="14px" />
                  <Skeleton variant="text" width="50%" height="11px" />
                </div>
              ))
            ) : stores.length === 0 ? (
              <div className="devmp-empty-sidebar">
                <p>No stores yet. Create one to get started.</p>
              </div>
            ) : (
              <ul className="devmp-store-list" role="list">
                {stores.map(store => (
                  <li key={store.id} role="listitem">
                    <button
                      className={`devmp-store-item${activeStore?.id === store.id ? ' active' : ''}`}
                      onClick={() => selectStore(store)}
                    >
                      <span className="store-item-name">{store.name}</span>
                      <span className="store-item-meta">
                        {store.game_title && <span className="store-game-tag">{store.game_title}</span>}
                        <span>{store.item_count ?? 0} items</span>
                      </span>
                      <span className="store-item-revenue">
                        {formatCurrency(store.revenue_usdc ?? 0)} revenue
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </aside>

          {/* ── Main: items panel ── */}
          <main className="devmp-main" id="main-content">
            {!activeStore ? (
              <div className="devmp-no-store">
                <EmptyState
                  icon={<StoreIcon />}
                  title="No store selected"
                  description="Select a store from the sidebar or create a new one."
                  action={
                    <button className="btn btn-primary" onClick={() => setShowCreateStore(true)}>
                      Create Store
                    </button>
                  }
                />
              </div>
            ) : (
              <>
                {/* Store meta header */}
                <div className="devmp-store-header">
                  <div>
                    <h2 className="devmp-store-name">{activeStore.name}</h2>
                    {activeStore.description && (
                      <p className="devmp-store-desc">{activeStore.description}</p>
                    )}
                  </div>
                  <div className="devmp-store-stats">
                    <div className="devmp-stat">
                      <span className="devmp-stat-label">Revenue (USDC)</span>
                      <span className="devmp-stat-val">{formatCurrency(activeStore.revenue_usdc ?? 0)}</span>
                    </div>
                    <div className="devmp-stat">
                      <span className="devmp-stat-label">Revenue (pts)</span>
                      <span className="devmp-stat-val">{(activeStore.revenue_points ?? 0).toLocaleString()} pts</span>
                    </div>
                    <div className="devmp-stat">
                      <span className="devmp-stat-label">Items</span>
                      <span className="devmp-stat-val">{storeItems?.length ?? activeStore.item_count ?? 0}</span>
                    </div>
                  </div>
                </div>

                {/* Items toolbar */}
                <div className="devmp-items-toolbar">
                  <h3 className="devmp-items-heading">Items</h3>
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={() => setShowAddItem(true)}
                  >
                    <span className="btn-icon" aria-hidden="true"><PlusIcon /></span>
                    Add Item
                  </button>
                </div>

                {/* What this node cannot do. Stated once, above the table,
                    rather than as controls that fail when clicked. */}
                <Unavailable
                  inline
                  headingLevel={3}
                  title="Some store actions are not built yet"
                  endpoints={[
                    'DELETE /api/v1/marketplace/stores/:id',
                    'DELETE /api/v1/marketplace/items/:id',
                  ]}
                >
                  A store and its items cannot be deleted on this node — no
                  route exists for either. Creating a store, adding and editing
                  items, revenue and purchases all work; set an item Inactive to
                  take it off sale.
                </Unavailable>

                {/* Items table */}
                {isLoadingItems ? (
                  <div className="devmp-items-loading">
                    {Array.from({ length: 3 }).map((_, i) => (
                      <Skeleton key={i} variant="rect" width="100%" height="60px" />
                    ))}
                  </div>
                ) : !storeItems || storeItems.length === 0 ? (
                  <EmptyState
                    icon={<PlusIcon />}
                    title="No items yet"
                    description="Add items to this store so players can purchase them."
                    action={
                      <button className="btn btn-primary" onClick={() => setShowAddItem(true)}>Add First Item</button>
                    }
                  />
                ) : (
                  <div className="devmp-items-table" role="table" aria-label="Store items">
                    <div className="devmp-table-header" role="row">
                      <span role="columnheader">Name</span>
                      <span role="columnheader">Type</span>
                      <span role="columnheader">Points</span>
                      <span role="columnheader">USDC</span>
                      <span role="columnheader">Sales</span>
                      <span role="columnheader">Status</span>
                      <span role="columnheader">Actions</span>
                    </div>
                    {storeItems.map(item => (
                      <div key={item.id} className="devmp-table-row" role="row">
                        <div className="devmp-item-name" role="cell">
                          <span className="item-name">{item.name}</span>
                          <span className="item-desc-sm">{item.description}</span>
                        </div>
                        <div role="cell">
                          <span className="item-type-badge">{item.item_type}</span>
                        </div>
                        <div role="cell" className="devmp-cell-mono">
                          {item.price_points.toLocaleString()} pts
                        </div>
                        <div role="cell" className="devmp-cell-mono">
                          {formatCurrency(item.price_usdc)}
                        </div>
                        <div role="cell" className="devmp-cell-mono">
                          {(item.sales ?? 0).toLocaleString()}
                        </div>
                        <div role="cell">
                          <span className={`item-status-badge ${item.active ? 'active' : 'inactive'}`}>
                            {item.active ? 'Active' : 'Inactive'}
                          </span>
                        </div>
                        <div role="cell" className="devmp-item-actions">
                          <button
                            className="icon-btn"
                            onClick={() => openEditItem(item)}
                            aria-label={`Edit ${item.name}`}
                          >
                            <PencilIcon />
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </>
            )}
          </main>
        </div>

        {/* ── Create store modal ── */}
        {showCreateStore && (
          <div className="devmp-overlay" role="dialog" aria-modal="true" aria-label="Create new store">
            <div className="devmp-modal">
              <h2 className="modal-title">Create Store</h2>
              <form onSubmit={handleCreateStore} className="devmp-form" noValidate>
                <div className="form-group">
                  <label htmlFor="store-name">Store Name <span aria-hidden="true">*</span></label>
                  <input
                    id="store-name"
                    type="text"
                    value={storeForm.name}
                    onChange={e => setStoreForm(f => ({ ...f, name: e.target.value }))}
                    placeholder="e.g. Cosmic Gear Shop"
                    required
                    autoFocus
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="store-game">Game ID (optional)</label>
                  <input
                    id="store-game"
                    type="text"
                    value={storeForm.game_id}
                    onChange={e => setStoreForm(f => ({ ...f, game_id: e.target.value }))}
                    placeholder="Leave blank for platform-wide"
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="store-desc">Description</label>
                  <textarea
                    id="store-desc"
                    value={storeForm.description}
                    onChange={e => setStoreForm(f => ({ ...f, description: e.target.value }))}
                    rows={3}
                    placeholder="What does your store sell?"
                  />
                </div>
                <div className="modal-actions">
                  <button type="button" className="btn btn-secondary" onClick={() => setShowCreateStore(false)}>
                    Cancel
                  </button>
                  <button type="submit" className="btn btn-primary" disabled={!storeForm.name || saving}>
                    {saving ? 'Creating…' : 'Create Store'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}

        {/* ── Add item modal ── */}
        {showAddItem && (
          <div className="devmp-overlay" role="dialog" aria-modal="true" aria-label="Add item">
            <div className="devmp-modal">
              <h2 className="modal-title">Add Item</h2>
              <form onSubmit={handleAddItem} className="devmp-form" noValidate>
                <ItemFormFields form={itemForm} onChange={setItemForm} />
                <div className="modal-actions">
                  <button type="button" className="btn btn-secondary" onClick={() => setShowAddItem(false)}>Cancel</button>
                  <button type="submit" className="btn btn-primary" disabled={!itemForm.name || saving}>
                    {saving ? 'Adding…' : 'Add Item'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}

        {/* ── Edit item modal ── */}
        {editingItem && (
          <div className="devmp-overlay" role="dialog" aria-modal="true" aria-label="Edit item">
            <div className="devmp-modal">
              <h2 className="modal-title">Edit Item — {editingItem.name}</h2>
              <form onSubmit={handleSaveItem} className="devmp-form" noValidate>
                <ItemFormFields form={itemForm} onChange={setItemForm} />
                <div className="modal-actions">
                  <button type="button" className="btn btn-secondary" onClick={() => setEditingItem(null)}>Cancel</button>
                  <button type="submit" className="btn btn-primary" disabled={!itemForm.name || saving}>
                    {saving ? 'Saving…' : 'Save Changes'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}

      </div>
    </Layout>
  );
}

// ── Shared item form fields ───────────────────────────────────────────────────

function ItemFormFields({ form, onChange }) {
  function set(field) {
    return e => onChange(f => ({ ...f, [field]: e.target.value }));
  }

  return (
    <>
      <div className="form-group">
        <label htmlFor="item-name">Name <span aria-hidden="true">*</span></label>
        <input
          id="item-name"
          type="text"
          value={form.name}
          onChange={set('name')}
          placeholder="e.g. Neon Rifle Skin"
          required
          autoFocus
        />
      </div>
      <div className="form-group">
        <label htmlFor="item-desc">Description</label>
        <textarea
          id="item-desc"
          value={form.description}
          onChange={set('description')}
          rows={2}
          placeholder="Short description shown to players"
        />
      </div>
      <div className="form-row">
        <div className="form-group">
          <label htmlFor="item-pts">Price (points)</label>
          <input
            id="item-pts"
            type="number"
            min="0"
            step="1"
            value={form.price_points}
            onChange={set('price_points')}
            placeholder="0"
          />
        </div>
        <div className="form-group">
          <label htmlFor="item-usdc">Price (USDC)</label>
          <input
            id="item-usdc"
            type="number"
            min="0"
            step="0.01"
            value={form.price_usdc}
            onChange={set('price_usdc')}
            placeholder="0.00"
          />
        </div>
      </div>
      <div className="form-group">
        <label htmlFor="item-type">Item Type</label>
        <select id="item-type" value={form.item_type} onChange={set('item_type')}>
          {ITEM_TYPES.map(t => (
            <option key={t} value={t}>{t.charAt(0).toUpperCase() + t.slice(1)}</option>
          ))}
        </select>
      </div>
    </>
  );
}
