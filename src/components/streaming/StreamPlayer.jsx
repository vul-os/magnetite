import { useEffect, useRef, useState, useCallback } from 'react';
import MessageList from '../comms/MessageList';
import MessageComposer from '../comms/MessageComposer';
import './StreamPlayer.css';

/**
 * StreamPlayer — HLS/WebRTC-ready <video> player with live chat sidebar.
 *
 * Props:
 *   stream   { id, title, game, streamer, viewerCount, hlsUrl?, webrtcOffer? }
 *   messages []          — chat messages for the stream channel
 *   onSend   (text)=>void
 *   onClose  ()=>void
 */
export default function StreamPlayer({ stream, messages = [], onSend, onClose }) {
  const videoRef = useRef(null);
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(true); // start muted to allow autoplay
  const [volume, setVolume] = useState(0.8);
  const [fullscreen, setFullscreen] = useState(false);
  const [showControls, setShowControls] = useState(true);
  const controlsTimerRef = useRef(null);
  const containerRef = useRef(null);

  const {
    title = 'Live Stream',
    game = '',
    streamer = 'Streamer',
    viewerCount = 0,
    hlsUrl,
  } = stream ?? {};

  // ── Video setup ──────────────────────────────────────────────────────────────
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    let hlsInstance = null;

    async function attachHls() {
      if (!hlsUrl) return;

      if (video.canPlayType('application/vnd.apple.mpegurl')) {
        // Native HLS — Safari, iOS. Set src directly.
        video.src = hlsUrl;
      } else {
        // Non-native HLS (Chrome, Firefox, Edge) — dynamically load hls.js
        const { default: Hls } = await import('hls.js');
        if (Hls.isSupported()) {
          hlsInstance = new Hls({ enableWorker: true, lowLatencyMode: true });
          hlsInstance.loadSource(hlsUrl);
          hlsInstance.attachMedia(video);
          hlsInstance.on(Hls.Events.MANIFEST_PARSED, () => {
            video.play().catch(() => null);
          });
        } else {
          // Fallback: try direct assignment (may not work without native HLS)
          video.src = hlsUrl;
        }
      }
    }

    attachHls();

    const onPlay  = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    video.addEventListener('play',  onPlay);
    video.addEventListener('pause', onPause);
    return () => {
      video.removeEventListener('play',  onPlay);
      video.removeEventListener('pause', onPause);
      if (hlsInstance) {
        hlsInstance.destroy();
        hlsInstance = null;
      }
    };
  }, [hlsUrl]);

  // ── Controls auto-hide ───────────────────────────────────────────────────────
  const showControlsTemporarily = useCallback(() => {
    setShowControls(true);
    clearTimeout(controlsTimerRef.current);
    controlsTimerRef.current = setTimeout(() => {
      if (playing) setShowControls(false);
    }, 3000);
  }, [playing]);

  useEffect(() => () => clearTimeout(controlsTimerRef.current), []);

  // ── Playback helpers ─────────────────────────────────────────────────────────
  const togglePlay = useCallback(() => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) v.play().catch(() => null);
    else v.pause();
  }, []);

  const toggleMute = useCallback(() => {
    const v = videoRef.current;
    if (!v) return;
    v.muted = !v.muted;
    setMuted(v.muted);
  }, []);

  const handleVolumeChange = useCallback((e) => {
    const val = Number(e.target.value);
    setVolume(val);
    if (videoRef.current) {
      videoRef.current.volume = val;
      videoRef.current.muted = val === 0;
      setMuted(val === 0);
    }
  }, []);

  const toggleFullscreen = useCallback(() => {
    if (!document.fullscreenElement) {
      containerRef.current?.requestFullscreen?.().then(() => setFullscreen(true)).catch(() => null);
    } else {
      document.exitFullscreen?.().then(() => setFullscreen(false)).catch(() => null);
    }
  }, []);

  useEffect(() => {
    const handler = () => setFullscreen(!!document.fullscreenElement);
    document.addEventListener('fullscreenchange', handler);
    return () => document.removeEventListener('fullscreenchange', handler);
  }, []);

  const formattedViewers =
    viewerCount >= 1000
      ? `${(viewerCount / 1000).toFixed(1)}k`
      : String(viewerCount);

  return (
    <section
      className="stream-player"
      aria-label={`Watching ${streamer}: ${title}`}
    >
      {/* ── Left: video pane ── */}
      <div
        ref={containerRef}
        className={`stream-player__video-pane${fullscreen ? ' stream-player__video-pane--fullscreen' : ''}`}
        onMouseMove={showControlsTemporarily}
        onMouseLeave={() => playing && setShowControls(false)}
      >
        {/* Back / close */}
        <button
          className="stream-player__back"
          onClick={onClose}
          aria-label="Back to streams"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polyline points="15 18 9 12 15 6" />
          </svg>
          <span>Back</span>
        </button>

        {/* Stream info overlay (top) */}
        <div className={`stream-player__info-overlay${showControls ? '' : ' stream-player__overlay--hidden'}`} aria-hidden="true">
          <span className="stream-player__live-badge">
            <span className="stream-player__live-dot" />
            LIVE
          </span>
          <span className="stream-player__viewer-count">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z" />
            </svg>
            {formattedViewers} watching
          </span>
        </div>

        {/* The video element */}
        <video
          ref={videoRef}
          className="stream-player__video"
          muted={muted}
          playsInline
          autoPlay={!!hlsUrl}
          aria-label={`Stream: ${title}`}
          onClick={togglePlay}
        >
          {!hlsUrl && (
            <p>
              Live stream will begin when the streamer goes live.
              <br />
              <small>HLS/WebRTC playback is ready to wire to a media server.</small>
            </p>
          )}
        </video>

        {/* No-source placeholder */}
        {!hlsUrl && (
          <div className="stream-player__placeholder" aria-hidden="true">
            <div className="stream-player__placeholder-icon">
              <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polygon points="23 7 16 12 23 17 23 7" />
                <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
              </svg>
            </div>
            <span className="kicker">// stream preview</span>
            <p className="stream-player__placeholder-text">
              {streamer} is live · Awaiting media source
            </p>
            <p className="stream-player__placeholder-sub">
              HLS or WebRTC source connects when the stream starts
            </p>
          </div>
        )}

        {/* Controls overlay (bottom) */}
        <div
          className={`stream-player__controls${showControls ? '' : ' stream-player__overlay--hidden'}`}
          role="toolbar"
          aria-label="Video controls"
        >
          {/* Play/pause */}
          <button
            className="stream-player__ctrl-btn"
            onClick={togglePlay}
            aria-label={playing ? 'Pause' : 'Play'}
          >
            {playing ? (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <rect x="6" y="4" width="4" height="16" rx="1" />
                <rect x="14" y="4" width="4" height="16" rx="1" />
              </svg>
            ) : (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            )}
          </button>

          {/* Volume */}
          <button
            className="stream-player__ctrl-btn"
            onClick={toggleMute}
            aria-label={muted ? 'Unmute' : 'Mute'}
          >
            {muted || volume === 0 ? (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
                <line x1="23" y1="9" x2="17" y2="15" />
                <line x1="17" y1="9" x2="23" y2="15" />
              </svg>
            ) : (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
                <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
                <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
              </svg>
            )}
          </button>

          <input
            className="stream-player__volume-slider"
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={muted ? 0 : volume}
            onChange={handleVolumeChange}
            aria-label="Volume"
          />

          <div className="stream-player__ctrl-spacer" />

          {/* Stream title */}
          <span className="stream-player__ctrl-title" aria-hidden="true">
            {streamer} · {game}
          </span>

          <div className="stream-player__ctrl-spacer" />

          {/* Fullscreen */}
          <button
            className="stream-player__ctrl-btn"
            onClick={toggleFullscreen}
            aria-label={fullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
          >
            {fullscreen ? (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polyline points="8 3 3 3 3 8" />
                <polyline points="21 3 16 3 16 8" />
                <polyline points="3 16 3 21 8 21" />
                <polyline points="16 21 21 21 21 16" />
              </svg>
            ) : (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polyline points="15 3 21 3 21 9" />
                <polyline points="9 21 3 21 3 15" />
                <line x1="21" y1="3" x2="14" y2="10" />
                <line x1="3" y1="21" x2="10" y2="14" />
              </svg>
            )}
          </button>
        </div>
      </div>

      {/* ── Right: live chat ── */}
      <aside className="stream-player__chat" aria-label="Live chat">
        <header className="stream-player__chat-header">
          <span className="kicker">// live chat</span>
          <h2 className="stream-player__chat-title">{title}</h2>
          <p className="stream-player__chat-sub">
            {streamer} · {game}
          </p>
        </header>

        <MessageList
          messages={messages}
          currentUserId="me"
        />

        <MessageComposer
          channel={{ name: 'stream-chat' }}
          onSend={onSend}
        />
      </aside>
    </section>
  );
}
