import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { InGameStore } from './InGameStore';

// Mock useMarketplace so we don't need a real API.
const mockPurchase = vi.fn();
const mockLoadItems = vi.fn();
const mockHasEntitlement = vi.fn();

const MOCK_ITEMS = [
  { id: 'igs1', name: 'Neon Helm Skin',    description: 'Glowing teal helmet.',      price_points: 800,  price_usdc: 0.99, item_type: 'cosmetic', active: true },
  { id: 'igs2', name: 'XP Accelerator',   description: '2× XP for 24 hours.',       price_points: 500,  price_usdc: 0.49, item_type: 'boost',    active: true },
  { id: 'igs3', name: 'Starter Bundle',   description: 'Skin + boost combo deal.',   price_points: 1200, price_usdc: 1.49, item_type: 'bundle',   active: true },
  { id: 'igs4', name: 'Point Pack (500)', description: 'Top-up 500 bonus points.',   price_points: 0,    price_usdc: 0.99, item_type: 'currency', active: true },
  { id: 'igs5', name: 'Inactive Item',    description: 'Not shown.',                 price_points: 100,  price_usdc: 0.10, item_type: 'cosmetic', active: false },
];

vi.mock('../../hooks/useMarketplace', () => ({
  useMarketplace: () => ({
    items: { 'test-store': MOCK_ITEMS },
    loadItems: mockLoadItems,
    purchase: mockPurchase,
    hasEntitlement: mockHasEntitlement,
    purchasing: false,
  }),
}));

describe('InGameStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockHasEntitlement.mockReturnValue(false);
    mockPurchase.mockResolvedValue({ success: true });
    mockLoadItems.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── Rendering ──────────────────────────────────────────────────────────────

  it('renders the panel with the correct game title', () => {
    render(<InGameStore storeId="test-store" gameTitle="Cosmic Raiders" onClose={() => {}} pointBalance={2500} />);
    expect(screen.getByText('Cosmic Raiders')).toBeInTheDocument();
  });

  it('renders the kicker label', () => {
    render(<InGameStore storeId="test-store" gameTitle="My Game" onClose={() => {}} pointBalance={0} />);
    expect(screen.getByText(/in-game store/i)).toBeInTheDocument();
  });

  it('shows item names for active items', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    expect(screen.getByText('Neon Helm Skin')).toBeInTheDocument();
    expect(screen.getByText('XP Accelerator')).toBeInTheDocument();
    expect(screen.getByText('Starter Bundle')).toBeInTheDocument();
  });

  it('does NOT show inactive items', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    expect(screen.queryByText('Inactive Item')).not.toBeInTheDocument();
  });

  it('shows the player point balance', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={3_500} />);
    expect(screen.getByText('3,500')).toBeInTheDocument();
  });

  it('shows the USDC balance', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={0} usdcBalance={12.50} />);
    expect(screen.getByText('$12.50')).toBeInTheDocument();
  });

  it('renders the close button and calls onClose on click', () => {
    const onClose = vi.fn();
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={onClose} pointBalance={0} />);
    fireEvent.click(screen.getByRole('button', { name: /close store/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('closes on Escape key press', () => {
    const onClose = vi.fn();
    const { container } = render(<InGameStore storeId="test-store" gameTitle="G" onClose={onClose} pointBalance={0} />);
    fireEvent.keyDown(container.firstChild, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('has correct ARIA attributes (dialog role, aria-modal, aria-label)', () => {
    render(<InGameStore storeId="test-store" gameTitle="Adventure Store" onClose={() => {}} pointBalance={0} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAttribute('aria-label', 'Adventure Store Store');
  });

  // ── Currency toggle ────────────────────────────────────────────────────────

  it('shows Points toggle pressed by default', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    const pointsBtn = screen.getByRole('button', { name: /points/i });
    expect(pointsBtn).toHaveAttribute('aria-pressed', 'true');
  });

  it('switches to USDC when USDC toggle is clicked', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} usdcBalance={20} />);
    fireEvent.click(screen.getByRole('button', { name: /usdc/i }));
    const usdcBtn = screen.getByRole('button', { name: /usdc/i });
    expect(usdcBtn).toHaveAttribute('aria-pressed', 'true');
  });

  it('shows point prices in points mode', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    // Neon Helm Skin: 800 pts
    expect(screen.getByText('800 pts')).toBeInTheDocument();
  });

  it('shows USDC prices after switching currency', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} usdcBalance={20} />);
    fireEvent.click(screen.getByRole('button', { name: /usdc/i }));
    // Multiple items may share the same price. Use getAllByText instead.
    const prices = screen.getAllByText('$0.99');
    expect(prices.length).toBeGreaterThan(0);
  });

  // ── Affordability ──────────────────────────────────────────────────────────

  it('shows "Buy" buttons when player has enough points', () => {
    // pointBalance=5000 covers all items (max cost 1200)
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    const buyButtons = screen.getAllByRole('button', { name: /buy .*/i });
    expect(buyButtons.length).toBeGreaterThan(0);
    buyButtons.forEach((btn) => expect(btn).not.toBeDisabled());
  });

  it('shows "Not Enough" label when player cannot afford an item', () => {
    // pointBalance=0 → cannot afford anything (except free items)
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={0} />);
    // Multiple items should show "Not Enough"
    const notEnough = screen.getAllByText('Not Enough');
    expect(notEnough.length).toBeGreaterThan(0);
  });

  it('shows "Free" for zero-price items', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={0} />);
    // Point Pack (500) has price_points=0 → "Free" in points mode
    expect(screen.getByText('Free')).toBeInTheDocument();
  });

  // ── Owned state ────────────────────────────────────────────────────────────

  it('shows "Owned" badge for items the player already has', () => {
    mockHasEntitlement.mockImplementation((id) => id === 'igs1');
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    expect(screen.getByLabelText(/already owned/i)).toBeInTheDocument();
  });

  it('does not show a Buy button for owned items', () => {
    mockHasEntitlement.mockImplementation((id) => id === 'igs1');
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);
    // igs1 → owned; the buy button for it should not exist
    expect(screen.queryByRole('button', { name: /buy neon helm skin/i })).not.toBeInTheDocument();
  });

  // ── Purchasing flow ────────────────────────────────────────────────────────

  it('calls purchase with the correct storeId, itemId, and currency', async () => {
    mockPurchase.mockResolvedValue({ success: true });
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);

    // Click Buy to open confirm modal
    const buyBtn = screen.getByRole('button', { name: /buy neon helm skin/i });
    fireEvent.click(buyBtn);

    // Confirm modal appears — click "Buy Now" to proceed
    const confirmBtn = await screen.findByRole('button', { name: /buy now/i });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(mockPurchase).toHaveBeenCalledWith('test-store', 'igs1', 'points');
    });
  });

  it('shows "Not Enough" buttons when player cannot afford items', () => {
    // pointBalance=50 → cannot afford items costing 500+ pts.
    // The button text content is "Not Enough" but aria-label is "Buy X for Y pts".
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={50} />);

    // Items costing more than 50 pts show "Not Enough" as the button text content.
    const notEnoughBtns = screen.getAllByText('Not Enough');
    expect(notEnoughBtns.length).toBeGreaterThan(0);
    // Each "Not Enough" element should be a disabled button.
    notEnoughBtns.forEach((el) => {
      const btn = el.closest('button') ?? el;
      expect(btn).toBeDisabled();
    });
  });

  it('shows "Purchased!" status briefly after successful purchase', async () => {
    mockPurchase.mockResolvedValue({ success: true });

    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);

    // Step 1: click Buy to open confirm modal
    const buyBtn = screen.getByRole('button', { name: /buy neon helm skin/i });
    fireEvent.click(buyBtn);

    // Step 2: click "Buy Now" to confirm
    const confirmBtn = await screen.findByRole('button', { name: /buy now/i });
    fireEvent.click(confirmBtn);

    // After the async purchase resolves, "Purchased!" text should appear.
    await waitFor(
      () => {
        expect(screen.getByText('Purchased!')).toBeInTheDocument();
      },
      { timeout: 3000 }
    );
  });

  // ── Type filter ────────────────────────────────────────────────────────────

  it('filters to show only selected type when type button is clicked', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);

    // Click "Boost" filter
    fireEvent.click(screen.getByRole('button', { name: /^boost$/i }));

    // Should show XP Accelerator (boost) but not Neon Helm Skin (cosmetic)
    expect(screen.getByText('XP Accelerator')).toBeInTheDocument();
    expect(screen.queryByText('Neon Helm Skin')).not.toBeInTheDocument();
  });

  it('shows all items when "All" filter is selected', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={5000} />);

    // Filter to boost first, then reset
    fireEvent.click(screen.getByRole('button', { name: /^boost$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^all$/i }));

    // All active items visible
    expect(screen.getByText('Neon Helm Skin')).toBeInTheDocument();
    expect(screen.getByText('XP Accelerator')).toBeInTheDocument();
  });

  // ── loadItems called on mount ──────────────────────────────────────────────

  it('calls loadItems with the storeId on mount', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={0} />);
    expect(mockLoadItems).toHaveBeenCalledWith('test-store');
  });

  // ── Footer ────────────────────────────────────────────────────────────────

  it('renders the purchases-are-final footer note', () => {
    render(<InGameStore storeId="test-store" gameTitle="G" onClose={() => {}} pointBalance={0} />);
    expect(screen.getByText(/purchases are final/i)).toBeInTheDocument();
  });
});
