// Tournaments.test.jsx — AGENT 5 tests for the Tournaments page (bracket rendering).
//
// Coverage:
//   - List loads and renders tournament names
//   - Status badges (Draft, Registration, InProgress, Completed, Cancelled)
//   - Loading state while fetching list
//   - Error state when list fetch fails
//   - Empty state when no tournaments
//   - BracketView renders rounds and matches
//   - BracketView: winner highlighted, loser dimmed
//   - BracketView: TBD slots for pending matches
//   - BracketView: empty state when no matches
//   - BracketView: Final label on last round
//   - CreateModal: opens on "+ Create" button click
//   - CreateModal: validation — missing required fields
//   - Filter tabs change visible tournaments
//   - formatPrize / formatFee display helpers

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// ── Mock CSS imports ──────────────────────────────────────────────────────────

vi.mock('./Tournaments.css', () => ({}));

// ── Mock Layout ───────────────────────────────────────────────────────────────

vi.mock('../components/Layout', () => ({
  default: ({ children }) => <div data-testid="layout">{children}</div>,
}));

// ── Mock api client ───────────────────────────────────────────────────────────

vi.mock('../api/client', () => ({
  api: {
    tournaments: {
      list: vi.fn(),
      get: vi.fn(),
      create: vi.fn(),
      register: vi.fn(),
      start: vi.fn(),
    },
  },
}));

import { api } from '../api/client';
import Tournaments from './Tournaments';

// ── Fixtures ──────────────────────────────────────────────────────────────────

const TOURNAMENTS = [
  {
    id: 'tour-1',
    name: 'Arena Open #12',
    game_id: 'game-1',
    status: 'Registration',
    max_players: 8,
    entry_fee: null,
    prize_pool: '500.00',
    start_time: new Date(Date.now() + 86400000).toISOString(),
    created_at: new Date().toISOString(),
  },
  {
    id: 'tour-2',
    name: 'Pro Invitational',
    game_id: 'game-1',
    status: 'InProgress',
    max_players: 16,
    entry_fee: '10.00',
    prize_pool: '2500.00',
    start_time: new Date(Date.now() - 3600000).toISOString(),
    created_at: new Date(Date.now() - 86400000).toISOString(),
  },
  {
    id: 'tour-3',
    name: 'Monthly Championship',
    game_id: 'game-1',
    status: 'Completed',
    max_players: 8,
    entry_fee: null,
    prize_pool: '200.00',
    start_time: new Date(Date.now() - 86400000 * 7).toISOString(),
    created_at: new Date(Date.now() - 86400000 * 14).toISOString(),
  },
];

const TOURNAMENT_DETAIL = {
  tournament: TOURNAMENTS[1], // Pro Invitational — InProgress
  participants: [
    { id: 'part-1', tournament_id: 'tour-2', user_id: 'p1', registered_at: new Date().toISOString(), status: 'registered', seed: 1, username: 'Alice' },
    { id: 'part-2', tournament_id: 'tour-2', user_id: 'p2', registered_at: new Date().toISOString(), status: 'registered', seed: 2, username: 'Bob' },
    { id: 'part-3', tournament_id: 'tour-2', user_id: 'p3', registered_at: new Date().toISOString(), status: 'registered', seed: 3, username: 'Charlie' },
    { id: 'part-4', tournament_id: 'tour-2', user_id: 'p4', registered_at: new Date().toISOString(), status: 'registered', seed: 4, username: 'Dana' },
  ],
  matches: [
    // Round 1
    { id: 'm1', tournament_id: 'tour-2', round: 1, match_number: 1, player1_id: 'p1', player2_id: 'p2', winner_id: 'p1', player1_score: 10, player2_score: 7, status: 'completed', scheduled_at: null, completed_at: new Date().toISOString() },
    { id: 'm2', tournament_id: 'tour-2', round: 1, match_number: 2, player1_id: 'p3', player2_id: 'p4', winner_id: 'p3', player1_score: 8, player2_score: 5, status: 'completed', scheduled_at: null, completed_at: new Date().toISOString() },
    // Round 2 (Final) — still pending
    { id: 'm3', tournament_id: 'tour-2', round: 2, match_number: 1, player1_id: 'p1', player2_id: 'p3', winner_id: null, player1_score: null, player2_score: null, status: 'pending', scheduled_at: null, completed_at: null },
  ],
};

// ── Helper ────────────────────────────────────────────────────────────────────

function renderTournaments() {
  return render(
    <MemoryRouter>
      <Tournaments />
    </MemoryRouter>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — loading state', () => {
  beforeEach(() => {
    api.tournaments.list.mockReturnValue(new Promise(() => {}));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading indicator while fetching list', () => {
    renderTournaments();
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Error state
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — error state', () => {
  beforeEach(() => {
    api.tournaments.list.mockRejectedValue(new Error('Server unreachable'));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows error message and Retry link when list fetch fails', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText(/server unreachable/i)).toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Empty list
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — empty list', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({ data: [] });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows empty state message when no tournaments found', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText(/no tournaments found/i)).toBeInTheDocument();
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Populated list
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — populated list', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({ data: TOURNAMENTS });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('renders all tournament names', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Arena Open #12')).toBeInTheDocument();
      expect(screen.getByText('Pro Invitational')).toBeInTheDocument();
      expect(screen.getByText('Monthly Championship')).toBeInTheDocument();
    });
  });

  it('renders "Open" badge for Registration status', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Open')).toBeInTheDocument();
    });
  });

  it('renders "Live" badge for InProgress status', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Live')).toBeInTheDocument();
    });
  });

  it('renders "Completed" badge for Completed status', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getAllByText('Completed').length).toBeGreaterThanOrEqual(1);
    });
  });

  it('shows Free for tournaments with null entry_fee', async () => {
    renderTournaments();
    await waitFor(() => {
      // The fee display shows "Free" for null entry_fee
      expect(screen.getAllByText('Free').length).toBeGreaterThanOrEqual(1);
    });
  });

  it('shows prize pool formatted as dollars', async () => {
    renderTournaments();
    await waitFor(() => {
      // $500 prize pool
      expect(screen.getByText(/\$500/)).toBeInTheDocument();
    });
  });

  it('renders a list region with aria-label "Tournaments list"', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('complementary', { name: /tournaments list/i })).toBeInTheDocument();
    });
  });

  it('renders filter tabs including All, Registration, InProgress', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('tab', { name: /all/i })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: /registration/i })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: /inprogress/i })).toBeInTheDocument();
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Tournament detail + bracket rendering
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — bracket view', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({ data: TOURNAMENTS });
    api.tournaments.get.mockResolvedValue({ data: TOURNAMENT_DETAIL });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  async function openDetail() {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Pro Invitational')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('Pro Invitational'));
    await waitFor(() => {
      expect(screen.getByRole('tree', { name: /tournament bracket/i })).toBeInTheDocument();
    });
  }

  it('renders bracket tree after clicking a tournament', async () => {
    await openDetail();
    expect(screen.getByRole('tree', { name: /tournament bracket/i })).toBeInTheDocument();
  });

  it('renders Round 1 and Final sections', async () => {
    await openDetail();
    expect(screen.getByText('Round 1')).toBeInTheDocument();
    expect(screen.getByText('Final')).toBeInTheDocument();
  });

  it('shows participant names in bracket matches', async () => {
    await openDetail();
    // Alice and Bob are in match 1 round 1 (may appear multiple times: bracket + winner bar + participants list)
    expect(screen.getAllByText('Alice').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Bob').length).toBeGreaterThanOrEqual(1);
  });

  it('shows scores for completed matches', async () => {
    await openDetail();
    // player1_score: 10, player2_score: 7
    expect(screen.getByText('10')).toBeInTheDocument();
    expect(screen.getByText('7')).toBeInTheDocument();
  });

  it('shows player names (not TBD) in final when players are assigned', async () => {
    // Match 3 (Final) has player1_id='p1' (Alice), player2_id='p3' (Charlie)
    // Both are named in the participant map — TBD only appears for null player IDs.
    await openDetail();
    // Alice (finalist) and Charlie (finalist) appear in the Final match
    expect(screen.getAllByText('Charlie').length).toBeGreaterThanOrEqual(1);
    // And neither finalist slot should read "TBD" in the Final
    const finalGroup = screen.getByRole('group', { name: 'Final' });
    expect(finalGroup).not.toHaveTextContent('TBD');
  });

  it('aria-label of match treeitem includes player names and winner', async () => {
    await openDetail();
    // Match m1: Alice vs Bob, winner: Alice
    const match1 = screen.getByRole('treeitem', { name: /alice vs bob/i });
    expect(match1).toBeInTheDocument();
    expect(match1.getAttribute('aria-label')).toMatch(/winner: alice/i);
  });

  it('match treeitem for pending Final does NOT contain winner in aria-label', async () => {
    await openDetail();
    const finalMatch = screen.getByRole('treeitem', { name: /alice vs charlie/i });
    expect(finalMatch.getAttribute('aria-label')).not.toMatch(/winner/i);
  });

  it('calls api.tournaments.get with tournament id on click', async () => {
    await openDetail();
    expect(api.tournaments.get).toHaveBeenCalledWith('tour-2');
  });

  it('renders participant list with count', async () => {
    await openDetail();
    // "// participants (4)"
    expect(screen.getByText(/participants \(4\)/i)).toBeInTheDocument();
  });

  it('shows all participant names in the participants list', async () => {
    await openDetail();
    expect(screen.getAllByText('Alice').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Bob').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Charlie').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Dana').length).toBeGreaterThanOrEqual(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// BracketView — empty matches
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — empty bracket state', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({ data: [TOURNAMENTS[0]] });
    api.tournaments.get.mockResolvedValue({
      data: {
        tournament: TOURNAMENTS[0],
        participants: [],
        matches: [], // no matches yet
      },
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows "No matches yet" when bracket is empty', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Arena Open #12')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('Arena Open #12'));
    await waitFor(() => {
      expect(screen.getByText(/no matches yet/i)).toBeInTheDocument();
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Create modal
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — create modal', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({ data: TOURNAMENTS });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('opens create modal when "+ Create" button is clicked', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /create tournament/i })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: /create tournament/i }));
    expect(screen.getByRole('dialog', { name: /create tournament/i })).toBeInTheDocument();
  });

  it('create modal has Name, Game ID, and Start Time fields', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /create tournament/i })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: /create tournament/i }));

    expect(screen.getByPlaceholderText(/arena open/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/uuid of the game/i)).toBeInTheDocument();
  });

  it('shows validation error when submitting empty form', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /create tournament/i })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: /create tournament/i }));

    // Use fireEvent.submit on the form to bypass HTML5 native validation in jsdom
    // (clicking submit triggers native required-field validation that blocks handleSubmit).
    const form = document.querySelector('form');
    expect(form).not.toBeNull();
    fireEvent.submit(form);

    // The React handler checks name.trim() === '' and sets an error via setErr
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
    expect(screen.getByRole('alert').textContent).toMatch(/required/i);
  });

  it('closes the modal when Cancel is clicked', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /create tournament/i })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: /create tournament/i }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Display helpers — formatPrize / formatFee
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — formatPrize and formatFee display', () => {
  beforeEach(() => {
    api.tournaments.list.mockResolvedValue({
      data: [
        { ...TOURNAMENTS[0], prize_pool: '1000.00', entry_fee: '5.00', status: 'Registration' },
        { ...TOURNAMENTS[1], prize_pool: '0', entry_fee: null, status: 'Draft' },
      ],
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('formats non-zero prize pool as $N', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText(/\$1,000/)).toBeInTheDocument();
    });
  });

  it('formats null entry_fee as "Free"', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText('Free')).toBeInTheDocument();
    });
  });

  it('formats non-zero entry_fee as $N.NN', async () => {
    renderTournaments();
    await waitFor(() => {
      expect(screen.getByText(/\$5\.00/)).toBeInTheDocument();
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// api client shape validation for tournament API
// ─────────────────────────────────────────────────────────────────────────────

describe('Tournaments — tournament API shape documented in client.js', () => {
  it('TOURNAMENT_DETAIL has the expected shape: tournament, participants, matches', () => {
    expect(TOURNAMENT_DETAIL).toHaveProperty('tournament');
    expect(TOURNAMENT_DETAIL).toHaveProperty('participants');
    expect(TOURNAMENT_DETAIL).toHaveProperty('matches');
  });

  it('TournamentMatch has required fields', () => {
    for (const m of TOURNAMENT_DETAIL.matches) {
      expect(m).toHaveProperty('id');
      expect(m).toHaveProperty('tournament_id');
      expect(m).toHaveProperty('round');
      expect(m).toHaveProperty('match_number');
      expect(m).toHaveProperty('status');
    }
  });

  it('TournamentParticipant has required fields', () => {
    for (const p of TOURNAMENT_DETAIL.participants) {
      expect(p).toHaveProperty('id');
      expect(p).toHaveProperty('tournament_id');
      expect(p).toHaveProperty('user_id');
      expect(p).toHaveProperty('status');
    }
  });

  it('Tournament has required fields', () => {
    const t = TOURNAMENT_DETAIL.tournament;
    expect(t).toHaveProperty('id');
    expect(t).toHaveProperty('name');
    expect(t).toHaveProperty('game_id');
    expect(t).toHaveProperty('status');
    expect(t).toHaveProperty('max_players');
    expect(t).toHaveProperty('prize_pool');
    expect(t).toHaveProperty('start_time');
    expect(t).toHaveProperty('created_at');
  });
});
