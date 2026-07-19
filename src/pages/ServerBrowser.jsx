import { useState } from 'react';
import Layout from '../components/Layout';
import { useDiscovery } from '../hooks/useDiscovery';
import { formatReceiptAmount, shortKey } from '../utils/currency';
import './ServerBrowser.css';

/**
 * Server browser — the flagship decentralized surface (seam §3.4 `Discovery`).
 *
 * Every row is a `SessionAd` a node published about *itself*. Nothing here was
 * assigned by a central scheduler: nodes measure their own hardware, decide
 * what they can host, name their own price, and announce. We just render the
 * phonebook. Anyone can add a row by running the `magnetite` binary.
 */

const PING_TIERS = [
  { max: 30, label: 'excellent' },
  { max: 80, label: 'good' },
  { max: 150, label: 'fair' },
];

function pingClass(ms) {
  return PING_TIERS.find((t) => ms <= t.max)?.label ?? 'poor';
}

function formatCapacity(capacity) {
  if (!capacity) return '—';
  const ram = Math.round((capacity.ram_mb ?? 0) / 1024);
  return `${capacity.cpu_cores}c · ${ram}GB · ${capacity.bandwidth_mbps}Mbps`;
}

function formatPrice(price) {
  if (!price) return 'Free';
  const unit = price.unit === 'per_hour' ? '/hr' : price.unit === 'per_seat' ? '/seat' : '';
  return `${formatReceiptAmount(price.amount, price.currency)}${unit}`;
}

export default function ServerBrowser() {
  const [gameFilter, setGameFilter] = useState('');
  const [freeSlotsOnly, setFreeSlotsOnly] = useState(false);
  const [freeOnly, setFreeOnly] = useState(false);
  const [maxPing, setMaxPing] = useState('');

  const { sessions, allSessions, games, loading, error } = useDiscovery({
    game: gameFilter || undefined,
    freeSlotsOnly,
    freeOnly,
    maxPing: maxPing ? Number(maxPing) : undefined,
  });

  const operators = new Set(allSessions.map((s) => s.node_operator)).size;
  const totalPlayers = allSessions.reduce((n, s) => n + (s.players ?? 0), 0);
  const totalSlots = allSessions.reduce((n, s) => n + (s.capacity?.free_slots ?? 0), 0);

  const hasFilters = gameFilter || freeSlotsOnly || freeOnly || maxPing;

  return (
    <Layout>
      <div className="browser">
        <header className="browser-header">
          <span className="kicker">// DISCOVERY</span>
          <h1>Server browser</h1>
          <p className="browser-subtitle">
            Nodes advertise themselves. Discovery is a phonebook, never an authority — no central
            scheduler decided any of this, and no permission was granted. Bring your own server and
            you appear here too.
          </p>
        </header>

        {/* ── Network summary ─────────────────────────────────────────────── */}
        <section className="browser-stats" aria-label="Network summary">
          <div className="stat">
            <span className="stat-value">{allSessions.length}</span>
            <span className="stat-label">sessions advertised</span>
          </div>
          <div className="stat">
            <span className="stat-value">{operators}</span>
            <span className="stat-label">independent operators</span>
          </div>
          <div className="stat">
            <span className="stat-value">{games.length}</span>
            <span className="stat-label">games by content hash</span>
          </div>
          <div className="stat">
            <span className="stat-value">{totalPlayers}</span>
            <span className="stat-label">players online</span>
          </div>
          <div className="stat">
            <span className="stat-value">{totalSlots}</span>
            <span className="stat-label">free slots</span>
          </div>
        </section>

        {/* ── BYO server ──────────────────────────────────────────────────── */}
        <section className="byo-card" aria-label="Bring your own server">
          <div className="byo-copy">
            <span className="kicker">// BRING YOUR OWN SERVER</span>
            <h2>Any box. No application, no allowlist.</h2>
            <p>
              A node measures its own hardware and advertises what it can actually hold, so player
              capacity is emergent from your box rather than a number we assign you. Run the binary,
              announce to any tracker, and you are in the list — charge for seats or host it free.
            </p>
          </div>
          <pre className="byo-code" aria-label="Commands to host a server">
            <code>
              {'$ magnetite node --game b3:7f41c0a8… \\\n'}
              {'    --announce tracker.example.org \\\n'}
              {'    --price 20usdc/hr\n'}
              {'\n'}
              {'  measured  32 cores · 128GB · 2000Mbps\n'}
              {'  shards    24 max, 11 live\n'}
              {'  announced ✓ visible to peers'}
            </code>
          </pre>
        </section>

        {/* ── Filters ─────────────────────────────────────────────────────── */}
        <section className="browser-filters" aria-label="Filter sessions">
          <div className="filter-field">
            <label htmlFor="sb-game">Game</label>
            <select id="sb-game" value={gameFilter} onChange={(e) => setGameFilter(e.target.value)}>
              <option value="">All games</option>
              {games.map((g) => (
                <option key={g.hash} value={g.hash}>
                  {g.title} ({g.nodes})
                </option>
              ))}
            </select>
          </div>

          <div className="filter-field">
            <label htmlFor="sb-ping">Max ping</label>
            <select id="sb-ping" value={maxPing} onChange={(e) => setMaxPing(e.target.value)}>
              <option value="">Any</option>
              <option value="30">≤ 30 ms</option>
              <option value="80">≤ 80 ms</option>
              <option value="150">≤ 150 ms</option>
            </select>
          </div>

          <label className="filter-toggle">
            <input
              type="checkbox"
              checked={freeSlotsOnly}
              onChange={(e) => setFreeSlotsOnly(e.target.checked)}
            />
            <span>Has free slots</span>
          </label>

          <label className="filter-toggle">
            <input type="checkbox" checked={freeOnly} onChange={(e) => setFreeOnly(e.target.checked)} />
            <span>No hosting fee</span>
          </label>

          {hasFilters && (
            <button
              type="button"
              className="btn btn-secondary filter-clear"
              onClick={() => {
                setGameFilter('');
                setFreeSlotsOnly(false);
                setFreeOnly(false);
                setMaxPing('');
              }}
            >
              Clear
            </button>
          )}
        </section>

        {error && (
          <div className="browser-alert" role="alert">
            {error} — discovery is redundant by design, so try another tracker or use LAN discovery.
          </div>
        )}

        {/* ── Session list ────────────────────────────────────────────────── */}
        {loading ? (
          <div className="browser-loading">
            <span className="spinner" aria-hidden="true" />
            <span>Querying trackers…</span>
          </div>
        ) : sessions.length === 0 ? (
          <div className="browser-empty">
            <h3>No sessions match</h3>
            <p>
              Nobody is advertising a session that fits. Relax the filters, point at another
              tracker, or host it yourself — the list is only as full as the peers you can reach.
            </p>
          </div>
        ) : (
          <div className="session-table" role="table" aria-label="Discovered sessions">
            <div className="session-row session-head" role="row">
              <span role="columnheader">Game / content address</span>
              <span role="columnheader">Node</span>
              <span role="columnheader">Players</span>
              <span role="columnheader">Capacity</span>
              <span role="columnheader">Ping</span>
              <span role="columnheader">Price</span>
              <span role="columnheader" className="sr-only">
                Join
              </span>
            </div>

            {sessions.map((s) => {
              const full = !(s.capacity?.free_slots > 0);
              return (
                <div className="session-row" role="row" key={s.id}>
                  <div className="cell-game" role="cell">
                    <span className="game-title">{s.game_title}</span>
                    <code className="game-hash" title={s.game}>
                      {shortKey(s.game.replace(/^b3:/, ''), 10, 6)}
                      <span className="hash-tag">blake3</span>
                    </code>
                    <span className="game-version">{s.version}</span>
                  </div>

                  <div className="cell-node" role="cell">
                    <span className="node-addr">{s.node}</span>
                    <span className="node-meta">
                      {s.node_operator} · {s.region}
                    </span>
                    <span className="node-comms">
                      {s.chat_room && <span className="comms-chip">chat</span>}
                      {s.voice_room && <span className="comms-chip">voice</span>}
                    </span>
                  </div>

                  <div className="cell-players" role="cell">
                    <span className="players-count">
                      {s.players}
                      <span className="players-max">/{s.max_players}</span>
                    </span>
                    <span className="players-bar" aria-hidden="true">
                      <span
                        className="players-fill"
                        style={{ width: `${Math.min(100, (s.players / s.max_players) * 100)}%` }}
                      />
                    </span>
                    <span className="players-slots">
                      {s.capacity?.free_slots ?? 0} free
                    </span>
                  </div>

                  <div className="cell-capacity" role="cell">
                    <code>{formatCapacity(s.capacity)}</code>
                    <span className="capacity-shards">{s.capacity?.max_shards} shards max</span>
                  </div>

                  <div className="cell-ping" role="cell">
                    <span className={`ping ping-${pingClass(s.ping_hint)}`}>{s.ping_hint} ms</span>
                  </div>

                  <div className="cell-price" role="cell">
                    <span className={s.price ? 'price-paid' : 'price-free'}>
                      {formatPrice(s.price)}
                    </span>
                    {s.price && <span className="price-note">paid to operator</span>}
                  </div>

                  <div className="cell-join" role="cell">
                    <button type="button" className="btn btn-primary btn-join" disabled={full}>
                      {full ? 'Full' : 'Join'}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        <p className="browser-footnote">
          Paid sessions settle wallet&rarr;wallet on the payment rail: joining mints a signed
          receipt the node checks before it lets you in. We never hold the funds and never sit
          between you and the operator.
        </p>
      </div>
    </Layout>
  );
}
