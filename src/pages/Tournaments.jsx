/**
 * Tournaments.jsx — list, create, join, and bracket view for tournaments.
 *
 * Endpoints used (via api.tournaments.*):
 *   GET  /api/v1/tournaments          — list
 *   GET  /api/v1/tournaments/:id      — details + bracket
 *   POST /api/v1/tournaments          — create
 *   POST /api/v1/tournaments/:id/register — join
 *   POST /api/v1/tournaments/:id/start    — start (generates bracket)
 */

import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';
import './Tournaments.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// ── Mock data ─────────────────────────────────────────────────────────────────

const MOCK_TOURNAMENTS = [
  {
    id: 'tour-1',
    name: 'Arena Open #12',
    game_id: 'game-1',
    status: 'Registration',
    max_players: 8,
    entry_fee: null,
    prize_pool: '500.00',
    start_time: new Date(Date.now() + 86400000 * 2).toISOString(),
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

function _makePlayer(n) {
  return { id: `p${n}`, username: `Player${n}`, seed: n };
}

const MOCK_DETAIL = {
  tournament: MOCK_TOURNAMENTS[1],
  participants: Array.from({ length: 4 }, (_, i) => ({
    id: `part-${i}`,
    tournament_id: 'tour-2',
    user_id: `p${i + 1}`,
    registered_at: new Date().toISOString(),
    status: 'registered',
    seed: i + 1,
    username: `Player${i + 1}`,
  })),
  matches: [
    // Round 1
    { id: 'm1', tournament_id: 'tour-2', round: 1, match_number: 1, player1_id: 'p1', player2_id: 'p2', winner_id: 'p1', player1_score: 10, player2_score: 7, status: 'completed', scheduled_at: null, completed_at: new Date().toISOString() },
    { id: 'm2', tournament_id: 'tour-2', round: 1, match_number: 2, player1_id: 'p3', player2_id: 'p4', winner_id: 'p3', player1_score: 8, player2_score: 5, status: 'completed', scheduled_at: null, completed_at: new Date().toISOString() },
    // Round 2 (Final)
    { id: 'm3', tournament_id: 'tour-2', round: 2, match_number: 1, player1_id: 'p1', player2_id: 'p3', winner_id: null, player1_score: null, player2_score: null, status: 'pending', scheduled_at: null, completed_at: null },
  ],
};

// ── Status helpers ────────────────────────────────────────────────────────────

const STATUS_META = {
  Draft:        { label: 'Draft',        cls: 'ts--draft' },
  Registration: { label: 'Open',         cls: 'ts--registration' },
  InProgress:   { label: 'Live',         cls: 'ts--inprogress' },
  Completed:    { label: 'Completed',    cls: 'ts--completed' },
  Cancelled:    { label: 'Cancelled',    cls: 'ts--cancelled' },
};

function StatusBadge({ status }) {
  const meta = STATUS_META[status] ?? { label: status, cls: '' };
  return <span className={`tournament-status ${meta.cls}`}>{meta.label}</span>;
}

function formatPrize(val) {
  if (!val) return '$0';
  return `$${parseFloat(val).toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 2 })}`;
}

function formatFee(val) {
  if (!val) return 'Free';
  return `$${parseFloat(val).toFixed(2)}`;
}

// ── Bracket ───────────────────────────────────────────────────────────────────

function BracketView({ matches, participants }) {
  if (!matches || matches.length === 0) {
    return (
      <div className="bracket-empty">
        <p>No matches yet. Start the tournament to generate the bracket.</p>
      </div>
    );
  }

  // Group by round
  const rounds = {};
  for (const m of matches) {
    if (!rounds[m.round]) rounds[m.round] = [];
    rounds[m.round].push(m);
  }
  const sortedRounds = Object.keys(rounds).map(Number).sort((a, b) => a - b);

  // Participant lookup by user_id
  const participantMap = {};
  for (const p of (participants || [])) {
    participantMap[p.user_id] = p.username || p.user_id?.slice(0, 8) || '—';
  }

  function playerName(pid) {
    if (!pid) return 'TBD';
    return participantMap[pid] || pid.slice(0, 8);
  }

  const totalRounds = sortedRounds.length;

  return (
    <div className="bracket-root" role="tree" aria-label="Tournament bracket">
      {sortedRounds.map((round, rIdx) => {
        const isLastRound = rIdx === totalRounds - 1;
        const roundLabel = isLastRound && totalRounds > 1 ? 'Final' : `Round ${round}`;
        return (
          <div key={round} className="bracket-round" role="group" aria-label={roundLabel}>
            <div className="bracket-round-label">{roundLabel}</div>
            <div className="bracket-matches">
              {rounds[round].map((m) => {
                const p1 = playerName(m.player1_id);
                const p2 = playerName(m.player2_id);
                const w = m.winner_id;
                return (
                  <div
                    key={m.id}
                    className={`bracket-match${m.status === 'completed' ? ' bracket-match--done' : ''}`}
                    role="treeitem"
                    aria-label={`${p1} vs ${p2}${w ? `, winner: ${playerName(w)}` : ''}`}
                  >
                    <div className={`bm-player${w === m.player1_id ? ' bm-player--winner' : w ? ' bm-player--loser' : ''}`}>
                      <span className="bm-name">{p1}</span>
                      {m.player1_score != null && <span className="bm-score">{m.player1_score}</span>}
                    </div>
                    <div className="bm-divider" aria-hidden="true">vs</div>
                    <div className={`bm-player${w === m.player2_id ? ' bm-player--winner' : w ? ' bm-player--loser' : ''}`}>
                      <span className="bm-name">{p2}</span>
                      {m.player2_score != null && <span className="bm-score">{m.player2_score}</span>}
                    </div>
                    {w && (
                      <div className="bm-winner-bar" aria-hidden="true">
                        <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                          <path d="M3 8l4 4 6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                        </svg>
                        {playerName(w)}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── Create modal ──────────────────────────────────────────────────────────────

function CreateModal({ onClose, onCreate }) {
  const [form, setForm] = useState({
    name: '',
    game_id: '',
    max_players: '8',
    entry_fee: '',
    prize_pool: '0',
    start_time: '',
  });
  const [submitting, setSubmitting] = useState(false);
  const [err, setErr] = useState(null);

  function handleChange(e) {
    setForm((f) => ({ ...f, [e.target.name]: e.target.value }));
  }

  async function handleSubmit(e) {
    e.preventDefault();
    if (!form.name.trim()) { setErr('Name is required.'); return; }
    if (!form.game_id.trim()) { setErr('Game ID is required.'); return; }
    if (!form.start_time) { setErr('Start time is required.'); return; }
    setErr(null);
    setSubmitting(true);
    try {
      const payload = {
        name: form.name.trim(),
        game_id: form.game_id.trim(),
        max_players: parseInt(form.max_players, 10) || 8,
        prize_pool: parseFloat(form.prize_pool) || 0,
        start_time: new Date(form.start_time).toISOString(),
        ...(form.entry_fee ? { entry_fee: parseFloat(form.entry_fee) } : {}),
      };
      const result = USE_MOCKS
        ? { ...payload, id: `tour-${Date.now()}`, status: 'Draft', created_at: new Date().toISOString() }
        : await api.tournaments.create(payload);
      onCreate(result?.data ?? result);
      onClose();
    } catch (ex) {
      setErr(ex.message || 'Failed to create tournament.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="t-modal-backdrop" role="dialog" aria-modal="true" aria-label="Create tournament">
      <div className="t-modal">
        <header className="t-modal-header">
          <h2 className="t-modal-title">// create tournament</h2>
          <button className="t-modal-close" onClick={onClose} aria-label="Close">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
            </svg>
          </button>
        </header>
        <form className="t-modal-body" onSubmit={handleSubmit}>
          {err && <div className="t-form-error" role="alert">{err}</div>}
          <label className="t-label">
            Name
            <input className="t-input" name="name" value={form.name} onChange={handleChange} placeholder="Arena Open #13" required />
          </label>
          <label className="t-label">
            Game ID
            <input className="t-input" name="game_id" value={form.game_id} onChange={handleChange} placeholder="uuid of the game" required />
          </label>
          <div className="t-form-row">
            <label className="t-label">
              Max Players
              <select className="t-input" name="max_players" value={form.max_players} onChange={handleChange}>
                {[2, 4, 8, 16, 32, 64].map((n) => <option key={n} value={n}>{n}</option>)}
              </select>
            </label>
            <label className="t-label">
              Entry Fee ($)
              <input className="t-input" name="entry_fee" type="number" min="0" step="0.01" value={form.entry_fee} onChange={handleChange} placeholder="0 = free" />
            </label>
          </div>
          <label className="t-label">
            Prize Pool ($)
            <input className="t-input" name="prize_pool" type="number" min="0" step="0.01" value={form.prize_pool} onChange={handleChange} />
          </label>
          <label className="t-label">
            Start Time
            <input className="t-input" name="start_time" type="datetime-local" value={form.start_time} onChange={handleChange} required />
          </label>
          <div className="t-modal-actions">
            <button type="button" className="t-btn t-btn--ghost" onClick={onClose} disabled={submitting}>Cancel</button>
            <button type="submit" className="t-btn t-btn--primary" disabled={submitting}>
              {submitting ? 'Creating…' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function Tournaments() {
  const [tournaments, setTournaments] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [selected, setSelected] = useState(null);        // tournament detail
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState(null);
  const [showCreate, setShowCreate] = useState(false);
  const [statusFilter, setStatusFilter] = useState('');
  const [joining, setJoining] = useState(false);
  const [starting, setStarting] = useState(false);
  const [actionError, setActionError] = useState(null);

  // Load tournament list
  const loadList = useCallback(() => {
    setLoading(true);
    setError(null);
    const params = statusFilter ? { status: statusFilter } : {};

    const fetch = USE_MOCKS
      ? Promise.resolve({ data: MOCK_TOURNAMENTS.filter((t) => !statusFilter || t.status === statusFilter) })
      : api.tournaments.list(params);

    fetch
      .then((res) => setTournaments(res?.data ?? res))
      .catch((err) => setError(err.message || 'Failed to load tournaments'))
      .finally(() => setLoading(false));
  }, [statusFilter]);

  useEffect(() => { loadList(); }, [loadList]);

  // Load tournament detail
  function openDetail(id) {
    setDetailLoading(true);
    setDetailError(null);
    setSelected(null);
    setActionError(null);

    const fetch = USE_MOCKS && id === 'tour-2'
      ? Promise.resolve({ data: MOCK_DETAIL })
      : USE_MOCKS
        ? Promise.resolve({ data: { tournament: MOCK_TOURNAMENTS.find((t) => t.id === id) || MOCK_TOURNAMENTS[0], participants: [], matches: [] } })
        : api.tournaments.get(id);

    fetch
      .then((res) => setSelected(res?.data ?? res))
      .catch((err) => setDetailError(err.message || 'Failed to load tournament'))
      .finally(() => setDetailLoading(false));
  }

  function handleCreated(t) {
    setTournaments((prev) => [t, ...prev]);
  }

  async function handleJoin(tournamentId) {
    setJoining(true);
    setActionError(null);
    try {
      if (!USE_MOCKS) await api.tournaments.register(tournamentId);
      // Refresh detail
      openDetail(tournamentId);
    } catch (ex) {
      setActionError(ex.message || 'Failed to join tournament');
    } finally {
      setJoining(false);
    }
  }

  async function handleStart(tournamentId) {
    setStarting(true);
    setActionError(null);
    try {
      if (!USE_MOCKS) {
        await api.tournaments.start(tournamentId);
      }
      openDetail(tournamentId);
      loadList();
    } catch (ex) {
      setActionError(ex.message || 'Failed to start tournament');
    } finally {
      setStarting(false);
    }
  }

  const STATUS_FILTERS = ['', 'Draft', 'Registration', 'InProgress', 'Completed', 'Cancelled'];

  return (
    <Layout>
      <div className="tournaments-page">
        {/* ── Sidebar list ── */}
        <aside className="tournaments-sidebar" aria-label="Tournaments list">
          <div className="tournaments-sidebar-header">
            <h1 className="tournaments-heading">
              <span className="tournaments-kicker">// tournaments</span>
            </h1>
            <button
              className="t-btn t-btn--primary t-btn--sm"
              onClick={() => setShowCreate(true)}
              aria-label="Create tournament"
            >
              + Create
            </button>
          </div>

          {/* Filter tabs */}
          <div className="tournaments-filters" role="tablist" aria-label="Filter by status">
            {STATUS_FILTERS.map((s) => (
              <button
                key={s || 'all'}
                role="tab"
                aria-selected={statusFilter === s}
                className={`t-filter-tab${statusFilter === s ? ' active' : ''}`}
                onClick={() => setStatusFilter(s)}
              >
                {s || 'All'}
              </button>
            ))}
          </div>

          {/* List */}
          {loading && (
            <div className="tournaments-loading" aria-live="polite">
              <div className="t-spinner" aria-hidden="true" />
              Loading…
            </div>
          )}
          {error && (
            <div className="tournaments-error" role="alert">
              {error}
              <button className="t-link" onClick={loadList}>Retry</button>
            </div>
          )}
          {!loading && !error && tournaments.length === 0 && (
            <div className="tournaments-empty">
              <p>No tournaments found.</p>
              <button className="t-btn t-btn--ghost t-btn--sm" onClick={() => setShowCreate(true)}>
                Create one
              </button>
            </div>
          )}
          <ul className="tournaments-list" role="list">
            {tournaments.map((t) => (
              <li key={t.id} role="listitem">
                <button
                  className={`tournaments-item${selected?.tournament?.id === t.id ? ' tournaments-item--active' : ''}`}
                  onClick={() => openDetail(t.id)}
                  aria-current={selected?.tournament?.id === t.id ? 'true' : undefined}
                >
                  <div className="ti-top">
                    <span className="ti-name">{t.name}</span>
                    <StatusBadge status={t.status} />
                  </div>
                  <div className="ti-meta">
                    <span className="ti-meta-item">
                      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                        <circle cx="8" cy="6" r="3" stroke="currentColor" strokeWidth="1.5" />
                        <path d="M2 14c0-3 2.5-5 6-5s6 2 6 5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                      </svg>
                      {t.max_players} max
                    </span>
                    <span className="ti-meta-item">
                      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                        <path d="M4 8h8M8 4v8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                        <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.5" />
                      </svg>
                      {formatPrize(t.prize_pool)}
                    </span>
                    <span className="ti-meta-item ti-fee">{formatFee(t.entry_fee)}</span>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        </aside>

        {/* ── Detail panel ── */}
        <main className="tournaments-detail" aria-label="Tournament details">
          {!selected && !detailLoading && !detailError && (
            <div className="tournaments-detail-empty">
              <svg width="48" height="48" viewBox="0 0 48 48" fill="none" aria-hidden="true">
                <rect x="8" y="14" width="32" height="22" rx="4" stroke="currentColor" strokeWidth="2" />
                <path d="M16 14v-3a8 8 0 0116 0v3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                <path d="M20 26l3 3 6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              <p>Select a tournament to view details and bracket.</p>
            </div>
          )}

          {detailLoading && (
            <div className="tournaments-detail-loading" aria-live="polite">
              <div className="t-spinner" aria-hidden="true" />
              Loading tournament…
            </div>
          )}

          {detailError && (
            <div className="tournaments-detail-error" role="alert">
              {detailError}
            </div>
          )}

          {selected && !detailLoading && (
            <div className="tournament-detail-inner">
              {/* Header */}
              <div className="td-header">
                <div>
                  <h2 className="td-title">{selected.tournament.name}</h2>
                  <div className="td-meta">
                    <StatusBadge status={selected.tournament.status} />
                    <span className="ti-meta-item">
                      {formatFee(selected.tournament.entry_fee)} entry
                    </span>
                    <span className="ti-meta-item">
                      {formatPrize(selected.tournament.prize_pool)} prize pool
                    </span>
                    <span className="ti-meta-item">
                      {selected.participants?.length ?? 0} / {selected.tournament.max_players} registered
                    </span>
                  </div>
                </div>
                <div className="td-actions">
                  {actionError && <span className="td-action-error" role="alert">{actionError}</span>}
                  {selected.tournament.status === 'Registration' && (
                    <button
                      className="t-btn t-btn--primary"
                      onClick={() => handleJoin(selected.tournament.id)}
                      disabled={joining}
                      aria-label="Register for this tournament"
                    >
                      {joining ? 'Joining…' : 'Join'}
                    </button>
                  )}
                  {selected.tournament.status === 'Registration' && (
                    <button
                      className="t-btn t-btn--ghost"
                      onClick={() => handleStart(selected.tournament.id)}
                      disabled={starting}
                      aria-label="Start tournament and generate bracket"
                    >
                      {starting ? 'Starting…' : 'Start'}
                    </button>
                  )}
                </div>
              </div>

              {/* Bracket */}
              <section aria-label="Bracket">
                <h3 className="td-section-title">// bracket</h3>
                <div className="td-bracket-wrap">
                  <BracketView matches={selected.matches} participants={selected.participants} />
                </div>
              </section>

              {/* Participants */}
              <section aria-label="Participants">
                <h3 className="td-section-title">// participants ({selected.participants?.length ?? 0})</h3>
                {(!selected.participants || selected.participants.length === 0) ? (
                  <p className="td-empty">No participants yet.</p>
                ) : (
                  <ol className="td-participants-list" aria-label="Registered participants">
                    {selected.participants.map((p, i) => (
                      <li key={p.id} className="td-participant">
                        <span className="td-p-seed">#{p.seed ?? i + 1}</span>
                        <span className="td-p-name">{p.username || p.user_id?.slice(0, 8) || '—'}</span>
                        <span className={`td-p-status${p.status === 'eliminated' ? ' ts--cancelled' : ''}`}>{p.status}</span>
                      </li>
                    ))}
                  </ol>
                )}
              </section>
            </div>
          )}
        </main>
      </div>

      {showCreate && (
        <CreateModal onClose={() => setShowCreate(false)} onCreate={handleCreated} />
      )}
    </Layout>
  );
}
