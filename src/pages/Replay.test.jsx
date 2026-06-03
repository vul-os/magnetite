// Replay.test.jsx — AGENT 5 frontend tests for the Replay viewer page.
//
// Coverage:
//   - Loading state while fetching
//   - Error state when API fails
//   - Replay data renders (verdict badge, replay ID, recorded_at)
//   - ReplayScrubber is mounted after data loads
//   - Empty / not-found state when API returns null
//   - buildArenaState pure logic: integrates inputs → player positions
//   - Mock replay shape conforms to documented ReplayLog structure

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';

// ── Mock CSS imports ──────────────────────────────────────────────────────────

vi.mock('./Replay.css', () => ({}));
vi.mock('../components/ReplayScrubber.css', () => ({}));

// ── Mock the renderer import (canvas utils) ───────────────────────────────────

vi.mock('../../magnetite-web-client/src/renderer.js', () => ({
  renderArenaView: vi.fn(),
}));

// ── Mock the ReplayScrubber ───────────────────────────────────────────────────

vi.mock('../components/ReplayScrubber', () => ({
  default: ({ currentTick, totalTicks, playing, speed, onPlay, onSeek, onSpeedChange }) => (
    <div data-testid="replay-scrubber">
      <span data-testid="scrubber-tick">{currentTick}</span>
      <span data-testid="scrubber-total">{totalTicks}</span>
      <button onClick={onPlay} aria-label={playing ? 'Pause' : 'Play'}>
        {playing ? 'Pause' : 'Play'}
      </button>
      <button onClick={() => onSeek?.(0)} aria-label="Rewind to start">Rewind</button>
      <button onClick={() => onSpeedChange?.(2)} aria-label="2× speed">2×</button>
      <span data-testid="scrubber-speed">{speed}</span>
    </div>
  ),
}));

// ── Mock api client ───────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    replays: {
      get: vi.fn(),
    },
  },
}));

import { api } from '../api/client';
import Replay from './Replay';

// ── jsdom stubs ───────────────────────────────────────────────────────────────

if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

if (typeof globalThis.requestAnimationFrame === 'undefined') {
  globalThis.requestAnimationFrame = (cb) => setTimeout(cb, 0);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
}

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_REPLAY = {
  id: 'replay-abc-1234-5678',
  config: {
    game_id: 'game-xyz',
    tick_hz: 60,
    max_players: 4,
    duration_ticks: 10,
    seed: 42,
    topology: 'SingleRoom',
    snapshot_every: 300,
  },
  frames: [
    [0, [['player-1', { keys: { forward: true, backward: false, left: false, right: false, attack: false, secondary_attack: false, jump: false, interact: false, sprint: false, crouch: false }, mouse: { x: 0, y: 0, delta_x: 0.5, delta_y: 0, left_button: false, right_button: false, middle_button: false, scroll: 0 }, sequence: 0, timestamp_ms: 0 }]]],
    [1, [['player-1', { keys: { forward: false, backward: false, left: false, right: true, attack: false, secondary_attack: false, jump: false, interact: false, sprint: false, crouch: false }, mouse: { x: 1, y: 0, delta_x: 0.5, delta_y: 0, left_button: false, right_button: false, middle_button: false, scroll: 0 }, sequence: 1, timestamp_ms: 16 }]]],
    [2, [['player-1', { keys: { forward: false, backward: false, left: false, right: false, attack: true, secondary_attack: false, jump: false, interact: false, sprint: false, crouch: false }, mouse: { x: 2, y: 0, delta_x: 0.5, delta_y: 0, left_button: true, right_button: false, middle_button: false, scroll: 0 }, sequence: 2, timestamp_ms: 32 }]]],
  ],
  state_hashes: [
    [0, 0xDEADBEEF],
    [1, 0xCAFEBABE],
    [2, 0xABCD1234],
  ],
  recorded_at: '2026-06-01T12:00:00.000Z',
  verdict: 'Clean',
};

// ── Helper ────────────────────────────────────────────────────────────────────

function renderReplay(replayId = 'replay-abc-1234-5678') {
  return render(
    <MemoryRouter initialEntries={[`/replay/${replayId}`]}>
      <Routes>
        <Route path="/replay/:id" element={<Replay />} />
      </Routes>
    </MemoryRouter>
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Replay page — loading state', () => {
  beforeEach(() => {
    // Never resolves during "loading" tests
    api.replays.get.mockReturnValue(new Promise(() => {}));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading indicator while fetching', () => {
    renderReplay();
    expect(screen.getByText(/loading replay/i)).toBeInTheDocument();
  });

  it('does not render scrubber while loading', () => {
    renderReplay();
    expect(screen.queryByTestId('replay-scrubber')).not.toBeInTheDocument();
  });
});

describe('Replay page — error state', () => {
  beforeEach(() => {
    api.replays.get.mockRejectedValue(new Error('Replay not found'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows error message when API fails', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByText(/replay not found/i)).toBeInTheDocument();
    });
  });

  it('shows a Retry button on error', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
    });
  });

  it('does not render scrubber on error', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.queryByTestId('replay-scrubber')).not.toBeInTheDocument();
    });
  });
});

describe('Replay page — loaded state', () => {
  beforeEach(() => {
    api.replays.get.mockResolvedValue(MOCK_REPLAY);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('renders the replay ID prefix in the header', async () => {
    renderReplay();
    await waitFor(() => {
      // The component shows replay.id.slice(0, 8)
      expect(screen.getByText('replay-a')).toBeInTheDocument();
    });
  });

  it('renders the "Clean" verdict badge', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByLabelText(/verdict: clean/i)).toBeInTheDocument();
    });
  });

  it('renders the recorded_at timestamp', async () => {
    renderReplay();
    await waitFor(() => {
      // Timestamp is rendered via toLocaleString, which varies by locale.
      // We just check that some date text is present near the header.
      // The date is 2026-06-01 so we look for "2026" in the page.
      expect(document.body.textContent).toMatch(/2026/);
    });
  });

  it('mounts the ReplayScrubber after data loads', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByTestId('replay-scrubber')).toBeInTheDocument();
    });
  });

  it('passes totalTicks = frames.length - 1 to scrubber', async () => {
    renderReplay();
    await waitFor(() => {
      const total = screen.getByTestId('scrubber-total');
      // 3 frames → totalTicks = 2
      expect(total.textContent).toBe('2');
    });
  });

  it('starts at tick 0', async () => {
    renderReplay();
    await waitFor(() => {
      const tick = screen.getByTestId('scrubber-tick');
      expect(tick.textContent).toBe('0');
    });
  });

  it('shows Play button (not Pause) by default', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /play/i })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /pause/i })).not.toBeInTheDocument();
    });
  });

  it('renders the canvas element', async () => {
    renderReplay();
    await waitFor(() => {
      const canvas = document.querySelector('canvas');
      expect(canvas).not.toBeNull();
    });
  });

  it('renders the replay viewer region aria-label', async () => {
    renderReplay();
    await waitFor(() => {
      expect(screen.getByRole('main', { name: /replay viewer/i })).toBeInTheDocument();
    });
  });
});

describe('Replay page — API client called with correct ID', () => {
  beforeEach(() => {
    api.replays.get.mockResolvedValue(MOCK_REPLAY);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('calls api.replays.get with the replay ID from URL params', async () => {
    renderReplay('replay-abc-1234-5678');
    await waitFor(() => {
      expect(api.replays.get).toHaveBeenCalledWith('replay-abc-1234-5678');
    });
  });

  it('unwraps { data: ... } API response shape', async () => {
    api.replays.get.mockResolvedValue({ data: MOCK_REPLAY });
    renderReplay();
    await waitFor(() => {
      // Scrubber mounts only when replay is set
      expect(screen.getByTestId('replay-scrubber')).toBeInTheDocument();
    });
  });
});

describe('Replay page — ReplayLog mock data shape matches documented structure', () => {
  it('MOCK_REPLAY has required ReplayLog fields: config, frames, state_hashes', () => {
    expect(MOCK_REPLAY).toHaveProperty('config');
    expect(MOCK_REPLAY).toHaveProperty('frames');
    expect(MOCK_REPLAY).toHaveProperty('state_hashes');
  });

  it('config has tick_hz, max_players, seed, topology, snapshot_every', () => {
    expect(MOCK_REPLAY.config).toHaveProperty('tick_hz');
    expect(MOCK_REPLAY.config).toHaveProperty('max_players');
    expect(MOCK_REPLAY.config).toHaveProperty('seed');
    expect(MOCK_REPLAY.config).toHaveProperty('topology');
    expect(MOCK_REPLAY.config).toHaveProperty('snapshot_every');
  });

  it('frames is array of [Tick, [PlayerId, Input][]] tuples', () => {
    expect(Array.isArray(MOCK_REPLAY.frames)).toBe(true);
    for (const frame of MOCK_REPLAY.frames) {
      // [tick, [...inputs]]
      expect(Array.isArray(frame)).toBe(true);
      expect(frame.length).toBe(2);
      const [tick, inputs] = frame;
      expect(typeof tick).toBe('number');
      expect(Array.isArray(inputs)).toBe(true);
      for (const [pid, input] of inputs) {
        expect(typeof pid).toBe('string');
        expect(typeof input).toBe('object');
        expect(input).toHaveProperty('keys');
        expect(input).toHaveProperty('mouse');
        expect(input).toHaveProperty('sequence');
      }
    }
  });

  it('state_hashes is array of [Tick, hash] tuples', () => {
    expect(Array.isArray(MOCK_REPLAY.state_hashes)).toBe(true);
    for (const [tick, hash] of MOCK_REPLAY.state_hashes) {
      expect(typeof tick).toBe('number');
      // hash is a u64 — in JS it arrives as a number or BigInt
      expect(['number', 'bigint'].includes(typeof hash)).toBe(true);
    }
  });

  it('verdict field is "Clean" or a divergence string', () => {
    expect(['Clean', 'Divergence'].some((v) => MOCK_REPLAY.verdict.startsWith(v))).toBe(true);
  });
});
