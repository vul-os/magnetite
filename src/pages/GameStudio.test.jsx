// GameStudio.test.jsx — unit tests for the Game Studio page (GDS feature).
//
// Tests cover the new 3-step flow:
//   Step 1 — Template gallery: renders templates, select one → go to step 2
//   Step 2 — Configure:        form validation + scaffold call
//   Step 3 — Result:           CLI instructions + next steps
//   A11y:                      aria labels, role="alert" on errors

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import GameStudio from './GameStudio';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockScaffold = vi.fn();
const mockTemplatesList = vi.fn();

vi.mock('../api/client', () => ({
  api: {
    templates: {
      list: (...args) => mockTemplatesList(...args),
    },
    developer: {
      scaffold: (...args) => mockScaffold(...args),
    },
  },
}));

// Stub Layout so tests don't need full router/context from it.
vi.mock('../components/Layout', () => ({
  default: ({ children }) => <div data-testid="layout">{children}</div>,
}));

// Stub GamePreview so it doesn't try to instantiate the web client.
vi.mock('../components/GamePreview', () => ({
  default: ({ devMode, onClose }) => (
    <div data-testid="game-preview">
      {devMode && <span>DEV PREVIEW</span>}
      {onClose && <button onClick={onClose}>Close Preview</button>}
    </div>
  ),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderStudio() {
  return render(
    <MemoryRouter>
      <GameStudio />
    </MemoryRouter>
  );
}

const MOCK_TEMPLATE = {
  id: 'arena-shooter',
  name: 'Arena Shooter',
  description: 'Top-down multiplayer arena shooter.',
  tier: 'free',
  tags: ['multiplayer', 'action'],
  player_count: '2–16',
  tick_hz: 60,
  topology: 'SingleRoom',
};

// ---------------------------------------------------------------------------
// Step 1 — Template gallery
// ---------------------------------------------------------------------------

describe('GameStudio — template gallery (step 1)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Return an empty list so the component uses built-in mock templates.
    mockTemplatesList.mockRejectedValue(new Error('not configured'));
  });

  it('renders the studio heading', () => {
    renderStudio();
    expect(screen.getByRole('heading', { name: /game studio/i })).toBeInTheDocument();
  });

  it('renders the kicker label', () => {
    renderStudio();
    expect(screen.getByText(/rust game studio/i)).toBeInTheDocument();
  });

  it('renders step 1 indicator as active', () => {
    renderStudio();
    // Step label text should be present
    expect(screen.getByText(/choose template/i)).toBeInTheDocument();
  });

  it('shows the template gallery heading', async () => {
    renderStudio();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /choose a template/i })).toBeInTheDocument();
    });
  });

  it('renders built-in templates when API fails', async () => {
    renderStudio();
    // The component falls back to built-in MOCK_TEMPLATES when the API fails.
    await waitFor(() => {
      expect(screen.getByText('Arena Shooter')).toBeInTheDocument();
    });
  });

  it('renders templates returned by the API', async () => {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    renderStudio();
    await waitFor(() => {
      expect(screen.getByText('Arena Shooter')).toBeInTheDocument();
    });
  });

  it('clicking a template advances to step 2', async () => {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    renderStudio();

    await waitFor(() => expect(screen.getByText('Arena Shooter')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Arena Shooter').closest('button'));

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /configure your game/i })).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// Step 2 — Configure
// ---------------------------------------------------------------------------

describe('GameStudio — configure step (step 2)', () => {
  async function goToStep2() {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    renderStudio();
    await waitFor(() => expect(screen.getByText('Arena Shooter')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Arena Shooter').closest('button'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /configure your game/i })).toBeInTheDocument()
    );
  }

  beforeEach(() => vi.clearAllMocks());

  it('renders the Game Name field', async () => {
    await goToStep2();
    expect(screen.getByLabelText(/game name/i)).toBeInTheDocument();
  });

  it('renders the Description field', async () => {
    await goToStep2();
    expect(screen.getByLabelText(/description/i)).toBeInTheDocument();
  });

  it('Create Game button is disabled when name is empty', async () => {
    await goToStep2();
    const btn = screen.getByRole('button', { name: /create game/i });
    expect(btn).toBeDisabled();
  });

  it('Create Game button becomes enabled when name is filled', async () => {
    await goToStep2();
    fireEvent.change(screen.getByLabelText(/game name/i), {
      target: { value: 'My Arena' },
    });
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /create game/i })).not.toBeDisabled()
    );
  });

  it('Back button returns to step 1 (template gallery)', async () => {
    await goToStep2();
    // Use the aria-label on the back navigation button (not the form "Back" text button).
    fireEvent.click(screen.getByRole('button', { name: /back to template gallery/i }));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /choose a template/i })).toBeInTheDocument()
    );
  });

  it('shows template name as context in step 2', async () => {
    await goToStep2();
    // Multiple elements reference "Arena Shooter" — check the section heading.
    expect(screen.getAllByText(/arena shooter/i).length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Scaffold flow
// ---------------------------------------------------------------------------

describe('GameStudio — scaffold flow', () => {
  async function goToConfigured(name = 'My Shooter') {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    renderStudio();
    await waitFor(() => expect(screen.getByText('Arena Shooter')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Arena Shooter').closest('button'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /configure your game/i })).toBeInTheDocument()
    );
    fireEvent.change(screen.getByLabelText(/game name/i), { target: { value: name } });
  }

  beforeEach(() => vi.clearAllMocks());

  it('calls api.developer.scaffold on submit', async () => {
    mockScaffold.mockResolvedValue({ game_id: 'gid-1', name: 'My Shooter' });
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() => expect(mockScaffold).toHaveBeenCalledOnce());
    const args = mockScaffold.mock.calls[0][0];
    expect(args.name).toBe('My Shooter');
    expect(args.template_id).toBe('arena-shooter');
  });

  it('advances to step 3 (result) after success', async () => {
    mockScaffold.mockResolvedValue({ game_id: 'gid-1', name: 'My Shooter' });
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /game created/i })).toBeInTheDocument()
    );
  });

  it('shows CLI instructions in the result step', async () => {
    mockScaffold.mockResolvedValue({ game_id: 'gid-1', name: 'My Shooter' });
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    // Multiple cli-block elements may contain "magnetite new" — check at least one exists.
    await waitFor(() => {
      const matches = screen.getAllByText(/magnetite new/i);
      expect(matches.length).toBeGreaterThan(0);
    });
  });

  it('shows error alert when scaffold fails', async () => {
    mockScaffold.mockRejectedValue(new Error('Server error'));
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert).toBeInTheDocument();
      expect(alert).toHaveTextContent(/server error/i);
    });
  });

  it('shows "Create Another Game" button in result step', async () => {
    mockScaffold.mockResolvedValue({ game_id: 'gid-1', name: 'My Shooter' });
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() =>
      expect(screen.getByRole('button', { name: /create another game/i })).toBeInTheDocument()
    );
  });

  it('"Create Another Game" resets back to step 1', async () => {
    mockScaffold.mockResolvedValue({ game_id: 'gid-1', name: 'My Shooter' });
    await goToConfigured('My Shooter');
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() =>
      expect(screen.getByRole('button', { name: /create another game/i })).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('button', { name: /create another game/i }));

    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /choose a template/i })).toBeInTheDocument()
    );
  });
});

// ---------------------------------------------------------------------------
// A11y
// ---------------------------------------------------------------------------

describe('GameStudio — accessibility', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockTemplatesList.mockRejectedValue(new Error('not configured'));
  });

  it('has an accessible page heading', async () => {
    renderStudio();
    expect(screen.getByRole('heading', { name: /game studio/i })).toBeInTheDocument();
  });

  it('form fields in step 2 have associated labels', async () => {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    renderStudio();

    await waitFor(() => expect(screen.getByText('Arena Shooter')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Arena Shooter').closest('button'));

    await waitFor(() => {
      expect(screen.getByLabelText(/game name/i)).toBeInTheDocument();
      expect(screen.getByLabelText(/description/i)).toBeInTheDocument();
    });
  });

  it('error state uses role="alert"', async () => {
    mockTemplatesList.mockResolvedValue([MOCK_TEMPLATE]);
    mockScaffold.mockRejectedValue(new Error('API error'));
    renderStudio();

    await waitFor(() => expect(screen.getByText('Arena Shooter')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Arena Shooter').closest('button'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /configure your game/i })).toBeInTheDocument()
    );
    fireEvent.change(screen.getByLabelText(/game name/i), { target: { value: 'Test' } });
    fireEvent.click(screen.getByRole('button', { name: /create game/i }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
  });
});
