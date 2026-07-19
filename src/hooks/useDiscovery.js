import { useState, useEffect, useMemo } from 'react';
import { api } from '../api/client';

/**
 * useDiscovery — the phonebook (seam §3.4 `Discovery`).
 *
 * Nodes *self-advertise* the sessions they host as `SessionAd`s. Discovery is a
 * swappable, redundant hint layer — it has no authority over who may play, what
 * a game is, or what a node charges. It replaces the old central
 * `runtime_instances` table + `/provisioning/pending` poll entirely.
 *
 * A game is identified by its **content address** (BLAKE3 hash of wasm +
 * manifest), not by a registry row, so two nodes advertising the same hash are
 * provably running the same build.
 */

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

/**
 * The canonical wire shape of one row of `GET /api/v1/discovery/sessions`.
 *
 * Field provenance — this is the whole point of the surface, so treat it as
 * load-bearing rather than decoration:
 *
 * | field | source | trust |
 * |---|---|---|
 * | `game` (plain BLAKE3 hex), `node`, `capacity`, `ping_hint`, `price`, `chat_room`, `voice_room` | the node | signed by `node_key` |
 * | `operator`, `region` | the node | signed, but **self-declared** — no tracker can verify where a box is or who runs it |
 * | `game_title`, `game_version` | the tracker's catalog | convenience lookup, **null for any hash it has not indexed** |
 * | `players`, `max_players` | the node | unsigned display counters |
 * | `id`, `node_key`, `expires_at` | the tracker | bookkeeping |
 *
 * `game` is **plain hex** — the `BLAKE3` chip in the UI is a rendering choice,
 * never part of the data. Every nullable field above really is null in
 * practice, so the UI must degrade rather than print `undefined`.
 */

// Mock ads — only used when VITE_USE_MOCKS === 'true'. Deterministic, so the
// screenshotter renders identical output with no tracker and no backend.
//
// These MIRROR the real payload exactly, including its nulls. Mocks that are
// richer than production are how a browser ends up rendering `undefined`
// against live data, so the last two rows deliberately exercise the degraded
// paths: a game this tracker has no catalog entry for, and a node that declared
// no operator/region.
const HASH_COSMIC = '7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de92f80b5537ea16c4d0938ab1';
const HASH_SPEED = '2ad9e6104b73fc85a01d7e29c4b850fa3e6d1927cc4f0b83a71e5d6294fb03c8';
const HASH_VOID = 'c184de0a7b29635f10e4a8cd7b3902fe58d64a1cbb730e95d2416af8073c5e29';

const MOCK_SESSIONS = [
  {
    id: '3f2a1b6c-0d4e-4a71-9c83-1e5f7a2b9d04',
    game: HASH_COSMIC,
    game_title: 'Cosmic Raiders',
    game_version: '1.4.2',
    node: 'nord-fjord-01.operator.net:7777',
    operator: 'nordfjord',
    region: 'eu-north',
    capacity: { cpu_cores: 32, ram_mb: 131072, bandwidth_mbps: 2000, free_slots: 46, max_shards: 24 },
    ping_hint: 18,
    price: { amount: 20, currency: 'USDC', unit: 'per_hour' },
    chat_room: 'builtin://room/cosmic-nord-01',
    voice_room: 'builtin://voice/cosmic-nord-01',
    node_key: 'a41f6b02c7d95e83104ab7cf2e6d0951b83c4a7e60d29f15caa3b78e4025d6f9',
    players: 82,
    max_players: 128,
    expires_at: 1800000120,
  },
  {
    id: '9b7c4d18-5e2f-4c0a-8d61-7b3e9f5a1c26',
    game: HASH_COSMIC,
    game_title: 'Cosmic Raiders',
    game_version: '1.4.2',
    node: 'home-rack.pareto.dev:7777',
    operator: 'pareto',
    region: 'self-hosted',
    capacity: { cpu_cores: 8, ram_mb: 32768, bandwidth_mbps: 500, free_slots: 11, max_shards: 4 },
    ping_hint: 7,
    price: null,
    chat_room: 'builtin://room/pareto-lan',
    voice_room: null,
    node_key: '5c93e07a1b4d6f28903ac5be7d14f062a97e3b58c026d4f19be7a350c8412e7d',
    players: 5,
    max_players: 16,
    expires_at: 1800000120,
  },
  {
    id: 'c15e8a90-2f76-4b3d-9e08-4a1c6d2b7f53',
    game: HASH_SPEED,
    game_title: 'Speed Legends',
    game_version: '2.0.0',
    node: 'sao-paulo-03.gridhost.io:7777',
    operator: 'gridhost',
    region: 'sa-east',
    capacity: { cpu_cores: 64, ram_mb: 262144, bandwidth_mbps: 5000, free_slots: 0, max_shards: 48 },
    ping_hint: 141,
    price: { amount: 15, currency: 'USDC', unit: 'per_hour' },
    chat_room: 'builtin://room/speed-sp-03',
    voice_room: 'builtin://voice/speed-sp-03',
    node_key: 'e820b47f5d1a396c02e7a8b34f95d106c73e2a9f480bd51e6ca29738f04b1d6a',
    players: 240,
    max_players: 240,
    expires_at: 1800000120,
  },
  {
    id: '7d3b0f24-8c19-4e5a-b076-2f8d1a4c9e35',
    game: HASH_SPEED,
    game_title: 'Speed Legends',
    game_version: '2.0.0',
    node: 'lan.local:7777',
    operator: 'you',
    region: 'lan',
    capacity: { cpu_cores: 12, ram_mb: 65536, bandwidth_mbps: 1000, free_slots: 14, max_shards: 8 },
    ping_hint: 1,
    price: null,
    chat_room: null,
    voice_room: null,
    node_key: '1a6fptr', // placeholder-short on purpose: keys vary in display width
    players: 2,
    max_players: 16,
    expires_at: 1800000120,
  },
  {
    // A game THIS tracker has never indexed. Perfectly normal in a
    // decentralized network: the content address is the identity, and the UI
    // falls back to it rather than inventing a name.
    id: 'b4e19c67-3a58-4d20-91cf-6e2b8d0a5347',
    game: HASH_VOID,
    game_title: null,
    game_version: null,
    node: 'frankfurt-11.metalcloud.eu:7777',
    operator: 'metalcloud',
    region: 'eu-central',
    capacity: { cpu_cores: 48, ram_mb: 196608, bandwidth_mbps: 4000, free_slots: 63, max_shards: 36 },
    ping_hint: 34,
    price: { amount: 25, currency: 'USDC', unit: 'per_hour' },
    chat_room: 'builtin://room/void-fra-11',
    voice_room: 'builtin://voice/void-fra-11',
    node_key: '6b02f9d4718ae35c0d94b7e2a681f350c47d9be208a1f6c35de907b41a2c8e6f',
    players: 129,
    max_players: 192,
    expires_at: 1800000120,
  },
  {
    // A node that declared no operator and no region, and publishes no
    // occupancy counters. Everything optional is genuinely absent here.
    id: 'e0a72d5b-9c34-4f18-86b2-3d7e1a9c4f60',
    game: HASH_VOID,
    game_title: null,
    game_version: null,
    node: 'tokyo-02.sakuranode.jp:7777',
    operator: null,
    region: null,
    capacity: { cpu_cores: 24, ram_mb: 98304, bandwidth_mbps: 1500, free_slots: 22, max_shards: 16 },
    ping_hint: 96,
    price: { amount: 12, currency: 'USDC', unit: 'per_hour' },
    chat_room: null,
    voice_room: null,
    node_key: '90c47ea1b6f2d385047ceb91a3f68d20b5e719ca4038f6d2ba81c07e35f9d248',
    players: null,
    max_players: null,
    expires_at: 1800000120,
  },
];

function asList(payload) {
  const body = payload?.data ?? payload;
  if (Array.isArray(body)) return body;
  if (Array.isArray(body?.sessions)) return body.sessions;
  if (Array.isArray(body?.ads)) return body.ads;
  return [];
}

/**
 * @param {object} filter
 * @param {string} [filter.game]      content address to narrow to
 * @param {number} [filter.maxPing]   drop ads slower than this
 * @param {boolean} [filter.freeSlotsOnly] drop full ads
 * @param {boolean} [filter.freeOnly] drop ads that charge a hosting fee
 */
export function useDiscovery(filter = {}) {
  const [sessions, setSessions] = useState(USE_MOCKS ? MOCK_SESSIONS : []);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const data = await api.discovery.sessions();
        if (!cancelled) setSessions(asList(data));
      } catch (err) {
        if (!cancelled) {
          // Discovery is a hint layer: a tracker being down is not fatal, it
          // just means we found nobody this time.
          setError(err.message || 'No discovery tracker reachable');
          setSessions([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  const { game, maxPing, freeSlotsOnly, freeOnly } = filter;

  const filtered = useMemo(() => {
    return sessions.filter((s) => {
      if (game && s.game !== game) return false;
      if (maxPing != null && s.ping_hint > maxPing) return false;
      if (freeSlotsOnly && !(s.capacity?.free_slots > 0)) return false;
      if (freeOnly && s.price) return false;
      return true;
    });
  }, [sessions, game, maxPing, freeSlotsOnly, freeOnly]);

  /**
   * Distinct games present in discovery, keyed by content address.
   *
   * `title` is nullable — the tracker may have no catalog entry for a hash, and
   * that is not an error. `label` is what a picker should show: the title when
   * one exists, otherwise the short content address, which is the game's real
   * identity anyway.
   */
  const games = useMemo(() => {
    const seen = new Map();
    for (const s of sessions) {
      if (!s.game) continue;
      if (!seen.has(s.game)) {
        seen.set(s.game, {
          hash: s.game,
          title: s.game_title ?? null,
          version: s.game_version ?? null,
          nodes: 0,
        });
      }
      const entry = seen.get(s.game);
      entry.nodes += 1;
      // First non-null title wins; trackers may resolve some rows and not others.
      if (entry.title == null && s.game_title != null) entry.title = s.game_title;
      if (entry.version == null && s.game_version != null) entry.version = s.game_version;
    }
    return [...seen.values()].map((g) => ({
      ...g,
      label: g.title ?? `${g.hash.slice(0, 10)}…`,
    }));
  }, [sessions]);

  return { sessions: filtered, allSessions: sessions, games, loading, error };
}
