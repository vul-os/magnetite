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
 *
 * The layout encodes provenance, because provenance is the product. Two
 * columns are content-addressed or key-signed and therefore checkable — those
 * carry the violet field rule. Everything from `Players` rightwards is a claim
 * the node makes about itself and nobody can confirm; that column boundary
 * carries the magenta rule and says so in words. No number on this page is
 * synthesised by the frontend: if the tracker did not send it, the cell says
 * it was not reported rather than showing a plausible figure.
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

/**
 * What a node *says about itself*: `operator · region`, either of which may be
 * absent. Nobody verifies these — a tracker cannot confirm that a box is in
 * Frankfurt or that the person running it is who they say. They are carried
 * inside the signed ad so they cannot be edited in flight, which is a different
 * (and much weaker) guarantee than being vouched for. The UI says so.
 */
function declaredBy(session) {
  return [session.operator, session.region].filter(Boolean).join(' · ');
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

  // Count distinct NODE KEYS, not self-declared operator names: a key is the
  // one thing here that is actually proven (every ad is signed by it), whereas
  // two nodes can call themselves the same operator, or none at all.
  const operators = new Set(allSessions.map((s) => s.node_key).filter(Boolean)).size;
  const totalPlayers = allSessions.reduce((n, s) => n + (s.players ?? 0), 0);
  const totalSlots = allSessions.reduce((n, s) => n + (s.capacity?.free_slots ?? 0), 0);

  const hasFilters = gameFilter || freeSlotsOnly || freeOnly || maxPing;

  function clearFilters() {
    setGameFilter('');
    setFreeSlotsOnly(false);
    setFreeOnly(false);
    setMaxPing('');
  }

  return (
    <Layout>
      <div className="sb">
        <header className="sb-head">
          <span className="kicker">// DISCOVERY</span>
          <h1 className="display-hero">Server browser</h1>
          <p className="sb-sub">
            Nodes advertise themselves. Discovery is a phonebook, never an authority — no central
            scheduler decided any of this, and no permission was granted. Bring your own server and
            you appear here too.
          </p>
        </header>

        {/* ── Network readout ─────────────────────────────────────────────────
            Derived only from the ads actually received. Nothing is estimated,
            extrapolated or held over from a previous poll. */}
        <section className="sb-readout" aria-label="Network summary">
          <div className="stat edge-field">
            <span className="stat-value">{allSessions.length}</span>
            <span className="stat-label m-sm">sessions advertised</span>
          </div>
          <div className="stat edge-field">
            <span className="stat-value">{operators}</span>
            <span className="stat-label m-sm">distinct node keys</span>
          </div>
          <div className="stat edge-field">
            <span className="stat-value">{games.length}</span>
            <span className="stat-label m-sm">games by content hash</span>
          </div>
          {/* Occupancy counters are unsigned display hints — the sum inherits
              exactly that weakness, so it is marked at the boundary. */}
          <div className="stat edge-boundary">
            <span className="stat-value">{totalPlayers}</span>
            <span className="stat-label m-sm">players reported</span>
          </div>
          <div className="stat edge-field">
            <span className="stat-value">{totalSlots}</span>
            <span className="stat-label m-sm">free slots</span>
          </div>
        </section>

        <p className="sb-legend">
          <span className="st st-field">checkable</span>
          <span className="sb-legend-note">signed by the node key or addressed by content hash</span>
          <span className="st st-boundary">unverifiable</span>
          <span className="sb-legend-note">carried in the ad, verifiable by nobody</span>
        </p>

        {/* ── BYO server ──────────────────────────────────────────────────── */}
        <section className="byo" aria-label="Bring your own server">
          <div className="byo-copy">
            <span className="kicker">// BRING YOUR OWN SERVER</span>
            <h2>Any box. No application, no allowlist.</h2>
            <p>
              A node measures its own hardware and advertises what it can actually hold, so player
              capacity is emergent from your box rather than a number we assign you. Run the binary,
              announce to any tracker, and you are in the list — charge for seats or host it free.
            </p>
          </div>
          <figure className="byo-fig">
            <figcaption className="m-sm byo-cap">example invocation</figcaption>
            <pre className="byo-code" aria-label="Commands to host a server">
              <code>
                {'$ magnetite node --game 7f41c0a8e35d92b6… \\\n'}
                {'    --announce tracker.example.org \\\n'}
                {'    --price 20usdc/hr\n'}
                {'\n'}
                {'  measured  32 cores · 128GB · 2000Mbps\n'}
                {'  shards    24 max, 11 live\n'}
                {'  announced ✓ lease renewed every 60s'}
              </code>
            </pre>
          </figure>
        </section>

        {/* ── Filters ─────────────────────────────────────────────────────── */}
        <form
          className="sb-filters"
          aria-label="Filter sessions"
          onSubmit={(e) => e.preventDefault()}
        >
          <div className="filter-field">
            <label className="m-sm" htmlFor="sb-game">
              Game
            </label>
            <select id="sb-game" value={gameFilter} onChange={(e) => setGameFilter(e.target.value)}>
              <option value="">All games</option>
              {games.map((g) => (
                <option key={g.hash} value={g.hash}>
                  {g.label} ({g.nodes})
                </option>
              ))}
            </select>
          </div>

          <div className="filter-field">
            <label className="m-sm" htmlFor="sb-ping">
              Max ping
            </label>
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
            <input
              type="checkbox"
              checked={freeOnly}
              onChange={(e) => setFreeOnly(e.target.checked)}
            />
            <span>No hosting fee</span>
          </label>

          <span className="filter-count m-sm" aria-live="polite">
            {loading ? 'querying' : `${sessions.length} of ${allSessions.length} shown`}
          </span>

          {hasFilters && (
            <button type="button" className="btn btn-secondary filter-clear" onClick={clearFilters}>
              Clear
            </button>
          )}
        </form>

        {/* ── Session list ────────────────────────────────────────────────── */}
        {loading ? (
          <div className="table-wrap sb-skeleton">
            <p className="sb-loading m-md" role="status">
              Querying trackers…
            </p>
            {[0, 1, 2, 3, 4].map((i) => (
              <span className="sk sk-row" key={i} aria-hidden="true" />
            ))}
          </div>
        ) : error ? (
          <div className="state state-error" role="alert">
            <p className="state-title">No tracker answered</p>
            <p className="state-body">
              {error} — discovery is redundant by design, so try another tracker or use LAN
              discovery. Nothing is cached and nothing is guessed, so the list is empty rather than
              stale.
            </p>
          </div>
        ) : sessions.length === 0 ? (
          <div className="state state-empty">
            <p className="state-title">No sessions match</p>
            <p className="state-body">
              Nobody is advertising a session that fits. Relax the filters, point at another
              tracker, or host it yourself — the list is only as full as the peers you can reach.
            </p>
            {hasFilters && (
              <div className="state-actions">
                <button type="button" className="btn btn-secondary" onClick={clearFilters}>
                  Clear filters
                </button>
              </div>
            )}
          </div>
        ) : (
          <div className="table-wrap">
            <table className="data sb-table">
              <caption className="sb-sr">
                Discovered sessions. Columns from Players onwards are claims the node makes about
                itself and are not verified by any tracker.
              </caption>
              <thead>
                <tr>
                  <th scope="col" className="edge-field">
                    Game / content address
                  </th>
                  <th scope="col">Node</th>
                  <th scope="col" className="edge-boundary">
                    Players
                  </th>
                  <th scope="col">Capacity</th>
                  <th scope="col">Ping</th>
                  <th scope="col">Price</th>
                  <th scope="col">
                    <span className="sb-sr">Join</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((s) => {
                  const full = !(s.capacity?.free_slots > 0);
                  const hash = s.game ?? '';
                  const shortHash = shortKey(hash, 10, 6);
                  // The tracker may not know this hash. The content address is
                  // the real identity, so fall back to it rather than to a
                  // placeholder — and when we do, show it ONCE rather than as
                  // both title and chip.
                  const titled = s.game_title != null;
                  const declared = declaredBy(s);
                  const hasCounts = s.players != null && s.max_players > 0;
                  return (
                    <tr key={s.id}>
                      <td className="lead cell-game edge-field">
                        {titled ? (
                          <>
                            <span className="game-title">{s.game_title}</span>
                            <code className="game-hash break-key" title={hash}>
                              {shortHash}
                              <span className="hash-tag m-xs">blake3</span>
                            </code>
                          </>
                        ) : (
                          <code className="game-title game-title-hash break-key" title={hash}>
                            {shortHash}
                            <span className="hash-tag m-xs">blake3</span>
                          </code>
                        )}
                        {s.game_version ? (
                          <span className="game-version">v{s.game_version}</span>
                        ) : (
                          <span className="game-version game-version-unknown">
                            not in this tracker&rsquo;s catalog
                          </span>
                        )}
                      </td>

                      <td className="key cell-node">
                        <span className="node-addr">{s.node}</span>
                        {declared ? (
                          <span
                            className="node-meta"
                            title="Declared by the node itself — signed by its key, but not verified by any tracker"
                          >
                            {declared}
                            <span className="declared-tag m-xs">self-declared</span>
                          </span>
                        ) : (
                          <span className="node-meta node-meta-none">no operator declared</span>
                        )}
                        <span className="node-comms">
                          {s.chat_room && <span className="comms-chip m-xs">chat</span>}
                          {s.voice_room && <span className="comms-chip m-xs">voice</span>}
                        </span>
                      </td>

                      <td className="num cell-players edge-boundary">
                        {hasCounts ? (
                          <>
                            <span className="players-count">
                              {s.players}
                              <span className="players-max">/{s.max_players}</span>
                            </span>
                            <span className="players-bar" aria-hidden="true">
                              <span
                                className="players-fill"
                                style={{
                                  width: `${Math.min(100, (s.players / s.max_players) * 100)}%`,
                                }}
                              />
                            </span>
                          </>
                        ) : (
                          <span
                            className="players-count players-count-unknown"
                            title="This node publishes no occupancy counters — they are unsigned display hints and entirely optional"
                          >
                            not reported
                          </span>
                        )}
                        <span className="players-slots">{s.capacity?.free_slots ?? 0} free</span>
                      </td>

                      <td className="num cell-capacity">
                        <span className="capacity-spec">{formatCapacity(s.capacity)}</span>
                        <span className="capacity-shards">{s.capacity?.max_shards ?? 0} shards max</span>
                      </td>

                      <td className="num cell-ping">
                        <span className={`ping ping-${pingClass(s.ping_hint ?? Infinity)}`}>
                          {s.ping_hint != null ? `${s.ping_hint} ms` : '—'}
                        </span>
                        <span className="ping-tier m-xs">
                          {s.ping_hint != null ? pingClass(s.ping_hint) : 'no hint'}
                        </span>
                      </td>

                      <td className="num cell-price">
                        <span className={s.price ? 'price-paid' : 'price-free'}>
                          {formatPrice(s.price)}
                        </span>
                        {s.price && <span className="price-note m-xs">paid to operator</span>}
                      </td>

                      <td className="cell-join">
                        <button type="button" className="btn btn-primary btn-join" disabled={full}>
                          {full ? 'Full' : 'Join'}
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        <p className="sb-footnote edge-boundary">
          Paid sessions settle wallet&rarr;wallet on the payment rail: joining mints a signed
          receipt the node checks before it lets you in. We never hold the funds and never sit
          between you and the operator — which also means the rail itself is outside anything we
          can verify for you.
        </p>
      </div>
    </Layout>
  );
}
