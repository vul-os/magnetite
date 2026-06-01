// GamePreview.test.jsx — unit tests for the GamePreview component.
//
// GamePreview connects to a magnetite-runtime WebSocket server using the
// magnetite-web-client.  These tests mock the web-client so they run
// entirely in jsdom without a live server.
//
// Coverage:
//   - Rendering: idle / devMode / connected / error / disconnected states
//   - Mock socket: Welcome → connected status
//   - Mock socket: Snapshot → updates player count via onState
//   - Mock socket: error → shows error overlay + role="alert"
//   - devMode: URL input + Connect button
//   - onClose callback
//   - Disconnect button
//   - wsEndpoint prop changes trigger re-connect

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import GamePreview from './GamePreview';

// ---------------------------------------------------------------------------
// Mock the magnetite-web-client createClient
// ---------------------------------------------------------------------------

// We build a fake MagnetiteClient factory so we can push messages in tests.
let _lastClient = null;

class FakeMagnetiteClient {
  constructor(opts) {
    this._opts = opts;
    this._stateListeners = new Set();
    this._conn = {
      onOpen: null,
      onClose: null,
      onError: null,
      isConnected: false,
      send: vi.fn(),
    };
    this._prediction = { lag: 12 };
    this._playerId = null;
    _lastClient = this;
  }

  connect() {
    this._conn.isConnected = true;
    return this;
  }

  disconnect() {
    this._conn.isConnected = false;
    if (this._conn.onClose) this._conn.onClose();
  }

  onState(fn) {
    this._stateListeners.add(fn);
    return () => this._stateListeners.delete(fn);
  }

  sendInput() {}

  // Helpers used by tests to simulate server messages.
  _triggerWelcome(playerId = '1', config = { tick_hz: 60 }) {
    this._playerId = String(playerId);
    this._handleWelcome({ player_id: playerId, config });
  }

  _handleWelcome(msg) {
    // GamePreview patches this after construction; we call the patched version.
    if (this.__patchedWelcome) this.__patchedWelcome(msg);
  }

  _triggerState(state) {
    for (const fn of this._stateListeners) {
      fn(state);
    }
  }

  _triggerError() {
    if (this._conn.onError) this._conn.onError();
  }

  _triggerClose() {
    this._conn.isConnected = false;
    if (this._conn.onClose) this._conn.onClose();
  }
}

vi.mock('../../magnetite-web-client/src/client.js', () => ({
  createClient: (opts) => new FakeMagnetiteClient(opts),
}));

// GamePreview imports CSS — mock it silently.
vi.mock('./GamePreview.css', () => ({}));

// jsdom does not implement ResizeObserver — stub it.
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderPreview(props = {}) {
  return render(
    <GamePreview title="Test Game" {...props} />
  );
}

// ---------------------------------------------------------------------------
// Rendering — idle state (no wsEndpoint, no devMode)
// ---------------------------------------------------------------------------

describe('GamePreview — idle state', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('renders with correct title', () => {
    renderPreview({ title: 'Space Arena' });
    expect(screen.getByText('Space Arena')).toBeInTheDocument();
  });

  it('shows the PREVIEW kicker label', () => {
    renderPreview();
    expect(screen.getByText(/preview/i)).toBeInTheDocument();
  });

  it('shows the "No Game Server" empty state when no endpoint and not devMode', () => {
    renderPreview({ wsEndpoint: null, devMode: false });
    expect(screen.getByText(/no game server/i)).toBeInTheDocument();
  });

  it('does NOT show canvas in idle state', () => {
    renderPreview({ wsEndpoint: null, devMode: false });
    const canvas = document.querySelector('canvas');
    if (canvas) {
      // Canvas is present but hidden
      expect(canvas.style.display).toBe('none');
    }
  });

  it('has a region role and aria-label', () => {
    renderPreview({ title: 'Neon Shooter' });
    expect(screen.getByRole('region')).toBeInTheDocument();
    expect(screen.getByRole('region')).toHaveAttribute(
      'aria-label',
      'Neon Shooter — game preview'
    );
  });

  it('shows no disconnect button in idle state', () => {
    renderPreview({ wsEndpoint: null });
    expect(screen.queryByRole('button', { name: /disconnect/i })).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Rendering — devMode
// ---------------------------------------------------------------------------

describe('GamePreview — devMode', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('shows the dev overlay with URL input', () => {
    renderPreview({ devMode: true, wsEndpoint: null });
    expect(screen.getByLabelText(/websocket server url/i)).toBeInTheDocument();
  });

  it('shows the Connect button in devMode', () => {
    renderPreview({ devMode: true, wsEndpoint: null });
    expect(screen.getByRole('button', { name: /connect/i })).toBeInTheDocument();
  });

  it('Connect button is disabled when URL is empty', () => {
    renderPreview({ devMode: true, wsEndpoint: null });
    const urlInput = screen.getByLabelText(/websocket server url/i);
    fireEvent.change(urlInput, { target: { value: '' } });
    expect(screen.getByRole('button', { name: /connect/i })).toBeDisabled();
  });

  it('Connect button is enabled when URL is non-empty', () => {
    renderPreview({ devMode: true, wsEndpoint: null });
    const urlInput = screen.getByLabelText(/websocket server url/i);
    fireEvent.change(urlInput, { target: { value: 'ws://localhost:9001' } });
    expect(screen.getByRole('button', { name: /connect/i })).not.toBeDisabled();
  });

  it('shows DEV PREVIEW kicker text', () => {
    renderPreview({ devMode: true, wsEndpoint: null });
    expect(screen.getByText(/dev preview/i)).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Connect flow — mock Welcome message
// ---------------------------------------------------------------------------

describe('GamePreview — connect flow', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('creates a client when wsEndpoint is provided', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());
  });

  it('shows "Connecting…" pill when wsEndpoint is given', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() =>
      expect(screen.queryByText(/connecting/i)).toBeInTheDocument()
    );
  });

  it('shows "Live" pill after a Welcome message', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });

    await waitFor(() => expect(_lastClient).not.toBeNull());

    // Patch the welcome handler (GamePreview replaces it).
    act(() => {
      // GamePreview patches _handleWelcome; trigger it via the patched version.
      _lastClient.__patchedWelcome = _lastClient._opts?.onWelcome || null;
      // Directly call the original patched slot that GamePreview set.
      if (_lastClient._handleWelcome) {
        _lastClient._handleWelcome({ player_id: '1', config: { tick_hz: 60 } });
      }
    });

    // After welcome the status should flip to connected.
    await waitFor(() =>
      expect(screen.queryByText(/live/i)).toBeInTheDocument()
    );
  });

  it('updates player count via onState callback', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });

    await waitFor(() => expect(_lastClient).not.toBeNull());

    // First send a Welcome to move to 'connected' state (player count only shows when connected).
    act(() => {
      if (_lastClient._handleWelcome) {
        _lastClient._handleWelcome({ player_id: '1', config: { tick_hz: 60 } });
      }
    });

    await waitFor(() => expect(screen.queryByText(/live/i)).toBeInTheDocument());

    act(() => {
      // Trigger a state update with 2 players (self + one other).
      _lastClient._triggerState({
        self_state: { id: '1', x: 0, y: 0, hp: 100, alive: true, angle: 0, score: 0, last_shot_tick: 0 },
        other_players: [
          { id: '2', x: 10, y: 10, hp: 100, alive: true, angle: 0, score: 0, last_shot_tick: 0 },
        ],
        projectiles: [],
        tick: 5,
      });
    });

    // Wait for "2 players" counter to appear.
    await waitFor(() =>
      expect(screen.queryByText(/2 players/i)).toBeInTheDocument()
    );
  });

  it('shows keyboard hint footer when connected', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      if (_lastClient._handleWelcome) {
        _lastClient._handleWelcome({ player_id: '1', config: { tick_hz: 60 } });
      }
    });

    await waitFor(() =>
      expect(screen.queryByText(/wasd/i)).toBeInTheDocument()
    );
  });
});

// ---------------------------------------------------------------------------
// Error state
// ---------------------------------------------------------------------------

describe('GamePreview — error state', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('shows error overlay with role="alert" on connection error', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      _lastClient._triggerError();
    });

    await waitFor(() => {
      const alerts = screen.getAllByRole('alert');
      expect(alerts.length).toBeGreaterThan(0);
    });
  });

  it('shows "Connection Failed" heading in error state', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      _lastClient._triggerError();
    });

    await waitFor(() =>
      expect(screen.queryByRole('heading', { name: /connection failed/i })).toBeInTheDocument()
    );
  });

  it('has "Try Again" button in error state', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      _lastClient._triggerError();
    });

    await waitFor(() =>
      expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
    );
  });

  it('"Try Again" resets to idle state', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      _lastClient._triggerError();
    });

    await waitFor(() =>
      expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('button', { name: /try again/i }));

    await waitFor(() =>
      expect(screen.queryByText(/connection failed/i)).not.toBeInTheDocument()
    );
  });
});

// ---------------------------------------------------------------------------
// Disconnected state
// ---------------------------------------------------------------------------

describe('GamePreview — disconnected state', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('shows disconnected overlay when server closes', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    // Simulate a Welcome first, then close.
    act(() => {
      if (_lastClient._handleWelcome) {
        _lastClient._handleWelcome({ player_id: '1', config: {} });
      }
    });
    act(() => {
      _lastClient._triggerClose();
    });

    // The overlay heading specifically says "Disconnected".
    await waitFor(() =>
      expect(screen.queryByRole('heading', { name: /^disconnected$/i })).toBeInTheDocument()
    );
  });

  it('shows role="alert" in disconnected state', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() => expect(_lastClient).not.toBeNull());

    act(() => {
      if (_lastClient._handleWelcome) {
        _lastClient._handleWelcome({ player_id: '1', config: {} });
      }
    });
    act(() => {
      _lastClient._triggerClose();
    });

    await waitFor(() => {
      const alerts = screen.getAllByRole('alert');
      expect(alerts.length).toBeGreaterThan(0);
    });
  });
});

// ---------------------------------------------------------------------------
// Disconnect button
// ---------------------------------------------------------------------------

describe('GamePreview — disconnect button', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('shows disconnect button while connecting', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /disconnect/i })).toBeInTheDocument()
    );
  });

  it('clicking disconnect resets to idle', async () => {
    renderPreview({ wsEndpoint: 'ws://localhost:9001' });
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('button', { name: /disconnect/i }));

    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /disconnect/i })).not.toBeInTheDocument()
    );
  });
});

// ---------------------------------------------------------------------------
// onClose callback
// ---------------------------------------------------------------------------

describe('GamePreview — onClose prop', () => {
  it('renders a close button when onClose is provided', () => {
    renderPreview({ onClose: vi.fn() });
    expect(screen.getByRole('button', { name: /close preview/i })).toBeInTheDocument();
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    renderPreview({ onClose });
    fireEvent.click(screen.getByRole('button', { name: /close preview/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does NOT render a close button when onClose is null', () => {
    renderPreview({ onClose: null });
    expect(screen.queryByRole('button', { name: /close preview/i })).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// wsEndpoint prop update triggers reconnect
// ---------------------------------------------------------------------------

describe('GamePreview — wsEndpoint prop updates', () => {
  beforeEach(() => {
    _lastClient = null;
    vi.clearAllMocks();
  });

  it('creates a new client when wsEndpoint changes from null to a URL', async () => {
    const { rerender } = renderPreview({ wsEndpoint: null });

    // No client created yet (no endpoint).
    expect(_lastClient).toBeNull();

    rerender(<GamePreview title="Test" wsEndpoint="ws://localhost:9001" />);

    await waitFor(() => expect(_lastClient).not.toBeNull());
  });
});
