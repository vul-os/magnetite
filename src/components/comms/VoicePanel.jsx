import { useEffect, useCallback } from 'react';
import { useVoiceClient } from '../../hooks/useVoiceClient';
import PresenceDot from './PresenceDot';
import './PresenceDot.css';
import './VoicePanel.css';

/**
 * VoicePanel — live voice channel panel (Wave 7).
 *
 * Props
 * -----
 * channel        {object|null}  — voice channel object ({ id, name, community_id? })
 * communityId    {string|null}  — community scope for room listing
 * onLeave        {function}     — called after leaveVoiceRoom completes
 *
 * Renders:
 *  - Connection state banner (idle / connecting / connected / failed / permission-denied)
 *  - Participant list with avatars, live speaking rings (AnalyserNode-driven), mute badges
 *  - Mute / Deafen / Leave controls wired to useVoiceClient
 *  - Graceful mic-permission-denied state
 *  - Reduced-motion safe (CSS guards the ring animations)
 */

// ── SVG icons ──────────────────────────────────────────────────────────────

function MicIcon({ muted }) {
  if (muted) {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <line x1="1" y1="1" x2="23" y2="23" />
        <path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6" />
        <path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23" />
        <line x1="12" y1="19" x2="12" y2="23" />
        <line x1="8" y1="23" x2="16" y2="23" />
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
      <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
      <line x1="12" y1="19" x2="12" y2="23" />
      <line x1="8" y1="23" x2="16" y2="23" />
    </svg>
  );
}

function HeadsetIcon({ deafened }) {
  if (deafened) {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <line x1="1" y1="1" x2="23" y2="23" />
        <path d="M3 18v-6a9 9 0 0 1 15-6.7M21 18v-6a9 9 0 0 0-1.11-4.37" />
        <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z" />
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 18v-6a9 9 0 0 1 18 0v6" />
      <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z" />
    </svg>
  );
}

function PhoneOffIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M10.68 13.31a16 16 0 0 0 3.41 2.6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7 2 2 0 0 1 1.72 2v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07C9.44 17.25 8.76 16.57 8.1 15.9" />
      <path d="M6.1 6.1A19.86 19.86 0 0 0 4.69 8.63 2 2 0 0 0 6.41 11a12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L5.39 17.2" />
      <line x1="23" y1="1" x2="1" y2="23" />
    </svg>
  );
}

function PhoneIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" stroke="none" aria-hidden="true">
      <path d="M6.62 10.79a15.05 15.05 0 0 0 6.59 6.59l2.2-2.2a1 1 0 0 1 1.01-.24 11.47 11.47 0 0 0 3.6.57 1 1 0 0 1 1 1V20a1 1 0 0 1-1 1A17 17 0 0 1 3 4a1 1 0 0 1 1-1h3.5a1 1 0 0 1 1 1 11.47 11.47 0 0 0 .57 3.6 1 1 0 0 1-.24 1.01z" />
    </svg>
  );
}

function AlertIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
  );
}

// ── Connection state helpers ───────────────────────────────────────────────

function peerStateLabel(state) {
  switch (state) {
    case 'connecting': return 'Connecting…';
    case 'connected':  return 'Voice Connected';
    case 'failed':     return 'Connection failed';
    case 'closed':     return 'Disconnected';
    default:           return 'Voice Ready';
  }
}

function peerStateClass(state) {
  switch (state) {
    case 'connected':  return 'voice-panel--connected';
    case 'connecting': return 'voice-panel--connecting';
    case 'failed':     return 'voice-panel--failed';
    default:           return '';
  }
}

// ── Participant row ────────────────────────────────────────────────────────

function ParticipantRow({ participant, isLocalSpeaking }) {
  const isSpeaking = participant.id === '__local__' ? isLocalSpeaking : !!participant.speaking;

  return (
    <div
      className={`voice-participant${isSpeaking ? ' voice-participant--speaking' : ''}`}
      title={[
        participant.username,
        participant.muted  ? '(muted)'   : '',
        isSpeaking         ? '— speaking' : '',
      ].filter(Boolean).join(' ')}
    >
      {/* Avatar + speaking ring */}
      <div className="voice-participant__avatar-wrap">
        <div className="voice-participant__avatar" aria-hidden="true">
          <span className="voice-participant__initial">
            {participant.username?.charAt(0).toUpperCase() ?? 'U'}
          </span>
        </div>
        {isSpeaking && (
          <span className="voice-participant__speaking-ring" aria-hidden="true" />
        )}
        <PresenceDot
          status={participant.status ?? 'online'}
          size="sm"
          className="voice-participant__dot"
        />
      </div>

      {/* Name */}
      <span className="voice-participant__name">{participant.username}</span>

      {/* Mute badge */}
      {participant.muted && (
        <span className="voice-participant__mute-icon" aria-label="Muted">
          <MicIcon muted />
        </span>
      )}
    </div>
  );
}

// ── Permission denied banner ───────────────────────────────────────────────

function PermissionBanner({ state }) {
  if (state === 'idle' || state === 'granted') return null;

  const msg = state === 'denied'
    ? 'Microphone access denied. Others can hear each other, but not you.'
    : 'No microphone found. Check your device settings.';

  return (
    <div className="voice-panel__permission-banner" role="alert">
      <AlertIcon />
      <span>{msg}</span>
    </div>
  );
}

// ── Main component ─────────────────────────────────────────────────────────

export default function VoicePanel({ channel, communityId, onLeave }) {
  const {
    currentRoom,
    participants,
    muted,
    deafened,
    toggleMute,
    toggleDeafen,
    peerState,
    permissionState,
    localSpeaking,
    joinVoiceRoom,
    leaveVoiceRoom,
    fetchRooms,
  } = useVoiceClient({ communityId });

  // Fetch room list when community is known
  useEffect(() => {
    if (communityId) fetchRooms();
  }, [communityId, fetchRooms]);

  // Auto-join this channel's voice room when channel changes
  useEffect(() => {
    if (!channel?.id) return;
    joinVoiceRoom(channel.id);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channel?.id]);

  const handleLeave = useCallback(() => {
    leaveVoiceRoom();
    if (onLeave) onLeave();
  }, [leaveVoiceRoom, onLeave]);

  if (!channel) return null;

  const isConnected  = peerState === 'connected';
  const isConnecting = peerState === 'connecting';
  const isFailed     = peerState === 'failed';
  const inRoom       = !!currentRoom;

  return (
    <div
      className={`voice-panel ${peerStateClass(peerState)}`}
      role="region"
      aria-label={`Voice: ${channel.name}`}
    >
      {/* Gradient top accent */}
      <div className="voice-panel__accent-bar" aria-hidden="true" />

      {/* Status header */}
      <div className="voice-panel__status">
        <span
          className={`voice-panel__indicator ${isConnecting ? 'voice-panel__indicator--pulsing' : ''}`}
          aria-hidden="true"
        />
        <div className="voice-panel__info">
          <span className="voice-panel__channel-name">{channel.name}</span>
          <span className="voice-panel__sub kicker">
            {inRoom
              ? peerStateLabel(peerState)
              : 'Click to join'
            }
            {inRoom && participants.length > 0
              ? ` · ${participants.length} connected`
              : ''
            }
          </span>
        </div>
        {/* Quick-join if not yet in this room */}
        {!inRoom && !isConnecting && (
          <button
            className="voice-panel__join-btn"
            onClick={() => joinVoiceRoom(channel.id)}
            aria-label={`Join voice channel ${channel.name}`}
          >
            <PhoneIcon />
            <span>Join</span>
          </button>
        )}
        {isConnecting && (
          <span className="voice-panel__spinner" aria-label="Connecting" />
        )}
        {isFailed && (
          <button
            className="voice-panel__join-btn voice-panel__join-btn--retry"
            onClick={() => joinVoiceRoom(channel.id)}
            aria-label="Retry connection"
          >
            Retry
          </button>
        )}
      </div>

      {/* Permission warning */}
      <PermissionBanner state={permissionState} />

      {/* Participant list */}
      {inRoom && participants.length > 0 && (
        <div className="voice-panel__participants" aria-label="Voice participants">
          {participants.map((p) => (
            <ParticipantRow
              key={p.id}
              participant={p}
              isLocalSpeaking={localSpeaking}
            />
          ))}
        </div>
      )}

      {/* Empty participants state while connected */}
      {inRoom && participants.length === 0 && isConnected && (
        <p className="voice-panel__empty">
          You&apos;re the only one here. Invite others!
        </p>
      )}

      {/* Controls — only shown when in a room */}
      {inRoom && (
        <div className="voice-panel__controls" aria-label="Voice controls">
          <button
            className={`voice-ctrl-btn${muted ? ' voice-ctrl-btn--active' : ''}`}
            onClick={toggleMute}
            aria-label={muted ? 'Unmute microphone' : 'Mute microphone'}
            aria-pressed={muted}
            title={muted ? 'Unmute' : 'Mute'}
          >
            <MicIcon muted={muted} />
            <span className="voice-ctrl-btn__label">{muted ? 'Unmute' : 'Mute'}</span>
          </button>

          <button
            className={`voice-ctrl-btn${deafened ? ' voice-ctrl-btn--active' : ''}`}
            onClick={toggleDeafen}
            aria-label={deafened ? 'Undeafen' : 'Deafen'}
            aria-pressed={deafened}
            title={deafened ? 'Undeafen' : 'Deafen'}
          >
            <HeadsetIcon deafened={deafened} />
            <span className="voice-ctrl-btn__label">{deafened ? 'Undeafen' : 'Deafen'}</span>
          </button>

          <button
            className="voice-ctrl-btn voice-ctrl-btn--leave"
            onClick={handleLeave}
            aria-label="Leave voice channel"
            title="Leave voice"
          >
            <PhoneOffIcon />
            <span className="voice-ctrl-btn__label">Leave</span>
          </button>
        </div>
      )}
    </div>
  );
}
