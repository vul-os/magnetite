import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import ServerBrowser from '../ServerBrowser';
import { api } from '../../api/client';

/**
 * Server-browser ↔ tracker contract.
 *
 * These tests feed the component the EXACT payload
 * `GET /api/v1/discovery/sessions` produces — flattened `SessionAd` plus tracker
 * bookkeeping — rather than a convenient frontend-shaped object. The bug class
 * they exist to prevent is the one where the mocks are richer than production
 * and the browser renders `undefined` against live data, so every nullable
 * field is exercised in its absent state.
 */

vi.mock('../../api/client', () => ({
  api: { discovery: { sessions: vi.fn() } },
}));

vi.mock('../../components/Layout', () => ({
  default: ({ children }) => <div>{children}</div>,
}));

const HASH = '7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de92f80b5537ea16c4d0938ab1';

/** A fully-populated row, exactly as the tracker serializes it. */
function fullRow(overrides = {}) {
  return {
    id: '3f2a1b6c-0d4e-4a71-9c83-1e5f7a2b9d04',
    game: HASH,
    game_title: 'Cosmic Raiders',
    game_version: '1.4.2',
    node: 'nord-fjord-01.operator.net:7777',
    operator: 'nordfjord',
    region: 'eu-north',
    capacity: {
      cpu_cores: 32,
      ram_mb: 131072,
      bandwidth_mbps: 2000,
      free_slots: 46,
      max_shards: 24,
    },
    ping_hint: 18,
    price: { amount: 20, currency: 'USDC', unit: 'per_hour' },
    chat_room: 'builtin://room/cosmic-nord-01',
    voice_room: null,
    node_key: 'a41f6b02c7d95e83104ab7cf2e6d0951b83c4a7e60d29f15caa3b78e4025d6f9',
    players: 82,
    max_players: 128,
    expires_at: 1800000120,
    ...overrides,
  };
}

/** The minimum a node can legally advertise: everything optional is null. */
function sparseRow() {
  return fullRow({
    id: 'e0a72d5b-9c34-4f18-86b2-3d7e1a9c4f60',
    game_title: null,
    game_version: null,
    operator: null,
    region: null,
    price: null,
    chat_room: null,
    voice_room: null,
    players: null,
    max_players: null,
  });
}

function serve(rows) {
  api.discovery.sessions.mockResolvedValue({
    success: true,
    data: { sessions: rows, total: rows.length },
  });
}

function renderBrowser() {
  return render(
    <MemoryRouter>
      <ServerBrowser />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('ServerBrowser ↔ discovery payload', () => {
  it('renders a fully-populated tracker row', async () => {
    serve([fullRow()]);
    renderBrowser();

    expect(await screen.findByText('Cosmic Raiders')).toBeInTheDocument();
    expect(screen.getByText('nord-fjord-01.operator.net:7777')).toBeInTheDocument();
    expect(screen.getByText('v1.4.2')).toBeInTheDocument();
    expect(screen.getByText('18 ms')).toBeInTheDocument();
  });

  it('never renders the string "undefined" for any row shape', async () => {
    serve([fullRow(), sparseRow()]);
    const { container } = renderBrowser();

    await screen.findByText('Cosmic Raiders');
    // The original bug: frontend expected game_title/node_operator/region/
    // version, which do not exist under those names, so cells read "undefined".
    expect(container.textContent).not.toMatch(/undefined/i);
    expect(container.textContent).not.toMatch(/\bNaN\b/);
    expect(container.textContent).not.toMatch(/\bnull\b/);
  });

  it('falls back to the content address when the tracker has no catalog entry', async () => {
    serve([sparseRow()]);
    const { container } = renderBrowser();

    await waitFor(() => expect(container.textContent).toMatch(/7f41c0a8/));
    // A hash nobody indexed is normal, and must be said plainly rather than
    // rendered as a blank or an invented title.
    expect(screen.getByText(/not in this tracker/i)).toBeInTheDocument();
  });

  it('labels operator/region as self-declared, never as verified', async () => {
    serve([fullRow()]);
    renderBrowser();

    expect(await screen.findByText(/nordfjord · eu-north/)).toBeInTheDocument();
    // The honesty requirement: a tracker cannot certify where a box is or who
    // runs it, so the UI must not present these as vouched-for.
    expect(screen.getByText(/self-declared/i)).toBeInTheDocument();
  });

  it('says so when a node declares no operator or region', async () => {
    serve([sparseRow()]);
    renderBrowser();
    expect(await screen.findByText(/no operator declared/i)).toBeInTheDocument();
  });

  it('degrades occupancy counters that a node did not publish', async () => {
    serve([sparseRow()]);
    renderBrowser();
    // players/max_players are unsigned display hints and entirely optional.
    expect(await screen.findByText(/not reported/i)).toBeInTheDocument();
    // free_slots IS signed, so it is still shown.
    expect(screen.getByText(/46 free/)).toBeInTheDocument();
  });

  it('counts distinct node keys rather than self-declared operator names', async () => {
    // Two rows claiming the same operator name, but different (proven) keys.
    serve([
      fullRow({ id: 'a', node_key: 'key-one', node: 'a.example:1' }),
      fullRow({ id: 'b', node_key: 'key-two', node: 'b.example:1' }),
    ]);
    renderBrowser();

    await screen.findAllByText('Cosmic Raiders');
    const label = screen.getByText('distinct node keys');
    expect(label.parentElement.textContent).toMatch(/2/);
  });

  it('treats plain hex as the canonical form (no b3: prefix expected)', async () => {
    serve([fullRow()]);
    const { container } = renderBrowser();

    await screen.findByText('Cosmic Raiders');
    expect(container.textContent).not.toMatch(/b3:/);
    // The short form of the plain hash is what is shown.
    expect(container.textContent).toMatch(/7f41c0a8/);
  });

  it('survives a tracker being unreachable without blanking the page', async () => {
    api.discovery.sessions.mockRejectedValue(new Error('No discovery tracker reachable'));
    renderBrowser();

    expect(await screen.findByRole('alert')).toHaveTextContent(/tracker/i);
  });
});
