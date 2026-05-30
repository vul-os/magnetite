import { useState, useEffect, useCallback } from 'react';
import Navbar from '../components/Navbar';
import StreamCard from '../components/streaming/StreamCard';
import StreamPlayer from '../components/streaming/StreamPlayer';
import GoLivePanel from '../components/streaming/GoLivePanel';
import { api } from '../api/client';
import './Streams.css';

// ── Mock stream data — only used when VITE_USE_MOCKS=true ───────────────────

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

const MOCK_STREAMS = [
  {
    id: 'stream-1',
    title: 'Building an FPS in Rust with Bevy — Live coding session',
    game: 'FPS Starter',
    streamer: 'rustdev42',
    viewerCount: 1420,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 72 * 60 * 1000).toISOString(),
    communityId: 'srv-1',
  },
  {
    id: 'stream-2',
    title: 'Motorsport physics deep-dive — rapier 3D colliders',
    game: 'Motorsport Demo',
    streamer: 'game_dev_mx',
    viewerCount: 847,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 38 * 60 * 1000).toISOString(),
    communityId: 'srv-1',
  },
  {
    id: 'stream-3',
    title: 'Bevy ECS patterns — component queries & system ordering',
    game: 'Bevy Engine',
    streamer: 'bevy_fan',
    viewerCount: 563,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 15 * 60 * 1000).toISOString(),
    communityId: 'srv-3',
  },
  {
    id: 'stream-4',
    title: 'WASM target compilation & Magnetite SDK integration',
    game: 'magnetite-sdk',
    streamer: 'wasm_wizard',
    viewerCount: 291,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
    communityId: 'srv-1',
  },
  {
    id: 'stream-5',
    title: 'Gamepad controller mapping in Rust — gilrs + SDK input layer',
    game: 'Controller Workshop',
    streamer: 'ferris_builds',
    viewerCount: 184,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 2 * 60 * 1000).toISOString(),
    communityId: 'srv-2',
  },
  {
    id: 'stream-6',
    title: 'Netcode from scratch — client prediction & rollback',
    game: 'Netcode Lab',
    streamer: 'async_alice',
    viewerCount: 922,
    thumbnailUrl: null,
    liveAt: new Date(Date.now() - 55 * 60 * 1000).toISOString(),
    communityId: 'srv-4',
  },
];

// Mock chat messages for a stream
function makeMockChatMessages(streamer) {
  const now = Date.now();
  return [
    {
      id: 'sc-1',
      channel_id: 'stream-chat',
      author: { id: 'u-1', username: 'viewer_alpha', display_name: 'ViewerAlpha' },
      authorId: 'u-1',
      content: `${streamer} is absolutely killing it today`,
      createdAt: new Date(now - 120_000).toISOString(),
      created_at: new Date(now - 120_000).toISOString(),
    },
    {
      id: 'sc-2',
      channel_id: 'stream-chat',
      author: { id: 'u-2', username: 'rust_fan', display_name: 'RustFan' },
      authorId: 'u-2',
      content: 'What crate are you using for the physics?',
      createdAt: new Date(now - 90_000).toISOString(),
      created_at: new Date(now - 90_000).toISOString(),
    },
    {
      id: 'sc-3',
      channel_id: 'stream-chat',
      author: { id: 'u-3', username: 'bevy_enjoyer', display_name: 'BevyEnjoyer' },
      authorId: 'u-3',
      content: 'rapier — same as in the fps-starter template',
      createdAt: new Date(now - 60_000).toISOString(),
      created_at: new Date(now - 60_000).toISOString(),
    },
    {
      id: 'sc-4',
      channel_id: 'stream-chat',
      author: { id: 'u-4', username: 'new_viewer', display_name: 'NewViewer' },
      authorId: 'u-4',
      content: 'First time watching, this platform is amazing!',
      createdAt: new Date(now - 20_000).toISOString(),
      created_at: new Date(now - 20_000).toISOString(),
    },
  ];
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function Streams() {
  const [streams, setStreams]           = useState(USE_MOCKS ? MOCK_STREAMS : []);
  const [loading, setLoading]           = useState(!USE_MOCKS);
  const [streamsError, setStreamsError] = useState(null);
  const [watchingStream, setWatching]   = useState(null);
  const [chatMessages, setChatMessages] = useState([]);
  const [showGoLive, setShowGoLive]     = useState(false);
  const [filter, setFilter]             = useState('');

  // ── Fetch streams ─────────────────────────────────────────────────────────────
  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function fetchStreams() {
      setLoading(true);
      setStreamsError(null);
      try {
        const data = await api.streams.list('global');
        if (!cancelled) {
          const result = Array.isArray(data?.streams)
            ? data.streams
            : (Array.isArray(data) ? data : []);
          setStreams(result);
        }
      } catch (err) {
        if (!cancelled) {
          setStreamsError(err.message ?? 'Failed to load streams');
          setStreams([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchStreams();
    return () => { cancelled = true; };
  }, []);

  // ── Watch stream ──────────────────────────────────────────────────────────────
  const handleWatch = useCallback(async (stream) => {
    setWatching(stream);
    setShowGoLive(false);
    // Seed chat with mocks only in mock mode; in production the comms WS feeds messages
    if (USE_MOCKS) {
      setChatMessages(makeMockChatMessages(stream.streamer));
    } else {
      setChatMessages([]);
    }

    // Try to resolve the real HLS/watch URL from the backend.
    try {
      const watchData = await api.streams.watch(stream.id).catch(() => null);
      if (watchData?.hls_url || watchData?.watch_url) {
        setWatching((prev) => prev
          ? { ...prev, hlsUrl: watchData.hls_url ?? watchData.watch_url }
          : prev
        );
      } else if (!stream.hlsUrl) {
        // Derive an HLS URL from the backend convention.
        const derivedUrl = api.streams.hlsUrl(stream.id);
        setWatching((prev) => prev ? { ...prev, hlsUrl: derivedUrl } : prev);
      }
    } catch {
      // Graceful — StreamPlayer shows a placeholder if hlsUrl is absent.
    }
  }, []);

  const handleCloseWatch = useCallback(() => {
    setWatching(null);
  }, []);

  const handleChatSend = useCallback((text) => {
    const msg = {
      id: `sc-${Date.now()}`,
      channel_id: 'stream-chat',
      author: { id: 'me', username: 'you', display_name: 'You' },
      authorId: 'me',
      content: text,
      createdAt: new Date().toISOString(),
      created_at: new Date().toISOString(),
    };
    setChatMessages((prev) => [...prev, msg]);
  }, []);

  // ── Go live ───────────────────────────────────────────────────────────────────
  const handleLive = useCallback((stream) => {
    setShowGoLive(false);
    setStreams((prev) => [
      { ...stream, liveAt: new Date().toISOString(), viewerCount: 0 },
      ...prev,
    ]);
  }, []);

  // ── Filter ────────────────────────────────────────────────────────────────────
  const filteredStreams = filter.trim()
    ? streams.filter((s) => {
        const q = filter.toLowerCase();
        return (
          s.title?.toLowerCase().includes(q) ||
          s.game?.toLowerCase().includes(q) ||
          s.streamer?.toLowerCase().includes(q)
        );
      })
    : streams;

  // ── Watch view ────────────────────────────────────────────────────────────────
  if (watchingStream) {
    return (
      <div className="streams-page">
        <Navbar />
        <main id="main-content" className="streams-watch-main">
          <StreamPlayer
            stream={watchingStream}
            messages={chatMessages}
            onSend={handleChatSend}
            onClose={handleCloseWatch}
          />
        </main>
      </div>
    );
  }

  // ── Browse grid ───────────────────────────────────────────────────────────────
  return (
    <div className="streams-page">
      <Navbar />

      <main id="main-content" className="streams-main bg-atmosphere">
        {/* ── Hero header ── */}
        <header className="streams-header reveal reveal-1">
          <div className="streams-header__left">
            <span className="kicker">// live now</span>
            <h1 className="streams-heading">
              Live Streams
            </h1>
            <p className="streams-subheading">
              Watch Rust game developers build, play, and ship — live.
            </p>
          </div>

          <div className="streams-header__right">
            <button
              className="btn btn-primary streams-golive-btn"
              onClick={() => setShowGoLive((v) => !v)}
              aria-expanded={showGoLive}
              aria-controls="golive-panel"
              aria-label="Go live"
            >
              <span className="streams-golive-dot" aria-hidden="true" />
              {showGoLive ? 'Cancel' : 'Go Live'}
            </button>
          </div>
        </header>

        {/* ── Go Live panel (expandable) ── */}
        {showGoLive && (
          <div id="golive-panel" className="streams-golive-section reveal reveal-2" aria-label="Go live setup">
            <GoLivePanel
              communityId={null}
              channelId={null}
              onLive={handleLive}
              onClose={() => setShowGoLive(false)}
            />
          </div>
        )}

        {/* ── Search / filter ── */}
        <div className="streams-filter reveal reveal-2">
          <div className="streams-search-wrap">
            <svg
              className="streams-search-icon"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
            <input
              className="streams-search"
              type="search"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter by title, game, or streamer…"
              aria-label="Search streams"
            />
          </div>

          <p className="streams-count" aria-live="polite" aria-atomic="true">
            {loading ? (
              <span className="streams-count--loading">Loading…</span>
            ) : (
              <>
                <span className="streams-count--num">{filteredStreams.length}</span>
                {' '}live
              </>
            )}
          </p>
        </div>

        {/* ── Error state ── */}
        {!loading && streamsError && (
          <div className="streams-empty reveal reveal-3" role="alert">
            <span className="kicker">// error</span>
            <p className="streams-empty__text">{streamsError}</p>
            <button
              className="btn btn-primary"
              onClick={() => window.location.reload()}
              aria-label="Retry loading streams"
            >
              Retry
            </button>
          </div>
        )}

        {/* ── Loading skeletons ── */}
        {loading && (
          <div className="streams-grid" aria-busy="true" aria-label="Loading streams">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="stream-skeleton" aria-hidden="true">
                <div className="stream-skeleton__thumb shimmer" />
                <div className="stream-skeleton__info">
                  <div className="stream-skeleton__avatar shimmer" />
                  <div className="stream-skeleton__text">
                    <div className="stream-skeleton__line stream-skeleton__line--title shimmer" />
                    <div className="stream-skeleton__line stream-skeleton__line--sub shimmer" />
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* ── Empty state ── */}
        {!loading && filteredStreams.length === 0 && (
          <div className="streams-empty reveal reveal-3">
            <div className="streams-empty__icon" aria-hidden="true">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <polygon points="23 7 16 12 23 17 23 7" />
                <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
              </svg>
            </div>
            <span className="kicker">// nothing here yet</span>
            <p className="streams-empty__text">
              {filter.trim()
                ? `No streams match "${filter}"`
                : 'No streams live right now'}
            </p>
            <button
              className="btn btn-primary"
              onClick={() => setShowGoLive(true)}
              aria-label="Go live now"
            >
              Be the first to go live
            </button>
          </div>
        )}

        {/* ── Stream browse grid ── */}
        {!loading && filteredStreams.length > 0 && (
          <div className="streams-grid reveal reveal-3" aria-label="Live streams">
            {filteredStreams.map((stream, i) => (
              <div
                key={stream.id}
                className="streams-grid__item"
                style={{ animationDelay: `${180 + i * 50}ms` }}
              >
                <StreamCard stream={stream} onWatch={handleWatch} />
              </div>
            ))}
          </div>
        )}

        {/* ── Platform info banner ── */}
        {!loading && (
          <aside className="streams-info-banner reveal reveal-5" aria-label="Streaming info">
            <div className="streams-info-banner__icon" aria-hidden="true">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div className="streams-info-banner__text">
              <strong>Streaming infrastructure</strong> — In-platform streams use WebRTC (mesh for small rooms;
              SFU via LiveKit/mediasoup at scale). RTMP egress relays to Twitch/YouTube with a stream key.
              HLS watch is served from the backend media pipeline.
            </div>
          </aside>
        )}
      </main>
    </div>
  );
}
