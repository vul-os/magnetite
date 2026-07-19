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

// Mock ads — only used when VITE_USE_MOCKS === 'true'. Deterministic, so the
// screenshotter renders identical output with no tracker and no backend.
const MOCK_SESSIONS = [
  {
    id: 'ad-1',
    game: 'b3:7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de',
    game_title: 'Cosmic Raiders',
    node: 'nord-fjord-01.operator.net:7777',
    node_operator: 'nordfjord',
    region: 'eu-north',
    capacity: { cpu_cores: 32, ram_mb: 131072, bandwidth_mbps: 2000, free_slots: 46, max_shards: 24 },
    players: 82,
    max_players: 128,
    ping_hint: 18,
    price: { amount: 20, currency: 'USDC', unit: 'per_hour' },
    chat_room: '#cosmic-raiders:matrix.org',
    voice_room: 'jitsi://meet.example.org/cosmic-nord-01',
    version: 'v1.4.2',
  },
  {
    id: 'ad-2',
    game: 'b3:7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de',
    game_title: 'Cosmic Raiders',
    node: 'home-rack.pareto.dev:7777',
    node_operator: 'pareto',
    region: 'self-hosted',
    capacity: { cpu_cores: 8, ram_mb: 32768, bandwidth_mbps: 500, free_slots: 11, max_shards: 4 },
    players: 5,
    max_players: 16,
    ping_hint: 7,
    price: null,
    chat_room: '#pareto-lan:matrix.org',
    voice_room: null,
    version: 'v1.4.2',
  },
  {
    id: 'ad-3',
    game: 'b3:2ad9e6104b73fc85a01d7e29c4b850fa3e6d1927',
    game_title: 'Speed Legends',
    node: 'sao-paulo-03.gridhost.io:7777',
    node_operator: 'gridhost',
    region: 'sa-east',
    capacity: { cpu_cores: 64, ram_mb: 262144, bandwidth_mbps: 5000, free_slots: 0, max_shards: 48 },
    players: 240,
    max_players: 240,
    ping_hint: 141,
    price: { amount: 15, currency: 'USDC', unit: 'per_hour' },
    chat_room: '#speed-legends:matrix.org',
    voice_room: 'livekit://sfu.gridhost.io/speed-sp-03',
    version: 'v2.0.0',
  },
  {
    id: 'ad-4',
    game: 'b3:2ad9e6104b73fc85a01d7e29c4b850fa3e6d1927',
    game_title: 'Speed Legends',
    node: 'lan.local:7777',
    node_operator: 'you',
    region: 'lan',
    capacity: { cpu_cores: 12, ram_mb: 65536, bandwidth_mbps: 1000, free_slots: 14, max_shards: 8 },
    players: 2,
    max_players: 16,
    ping_hint: 1,
    price: null,
    chat_room: null,
    voice_room: null,
    version: 'v2.0.0',
  },
  {
    id: 'ad-5',
    game: 'b3:c184de0a7b29635f10e4a8cd7b3902fe58d64a1c',
    game_title: 'Void Tactics',
    node: 'frankfurt-11.metalcloud.eu:7777',
    node_operator: 'metalcloud',
    region: 'eu-central',
    capacity: { cpu_cores: 48, ram_mb: 196608, bandwidth_mbps: 4000, free_slots: 63, max_shards: 36 },
    players: 129,
    max_players: 192,
    ping_hint: 34,
    price: { amount: 25, currency: 'USDC', unit: 'per_hour' },
    chat_room: '#void-tactics:matrix.org',
    voice_room: 'jitsi://meet.example.org/void-fra-11',
    version: 'v0.9.7',
  },
  {
    id: 'ad-6',
    game: 'b3:c184de0a7b29635f10e4a8cd7b3902fe58d64a1c',
    game_title: 'Void Tactics',
    node: 'tokyo-02.sakuranode.jp:7777',
    node_operator: 'sakuranode',
    region: 'ap-northeast',
    capacity: { cpu_cores: 24, ram_mb: 98304, bandwidth_mbps: 1500, free_slots: 22, max_shards: 16 },
    players: 41,
    max_players: 64,
    ping_hint: 96,
    price: { amount: 12, currency: 'USDC', unit: 'per_hour' },
    chat_room: '#void-tactics:matrix.org',
    voice_room: null,
    version: 'v0.9.7',
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

  /** Distinct games present in discovery, keyed by content address. */
  const games = useMemo(() => {
    const seen = new Map();
    for (const s of sessions) {
      if (!seen.has(s.game)) seen.set(s.game, { hash: s.game, title: s.game_title, nodes: 0 });
      seen.get(s.game).nodes += 1;
    }
    return [...seen.values()];
  }, [sessions]);

  return { sessions: filtered, allSessions: sessions, games, loading, error };
}
