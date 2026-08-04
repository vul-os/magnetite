/**
 * Shared domain types for the Wave 6 comms feature (communities, channels,
 * messages, presence, voice) and the sockets that drive them.
 *
 * These mirror the backend's snake_case JSON shapes exactly — the frontend
 * does not rename fields, so the types below use snake_case where the wire
 * format does.
 */

export interface Community {
  id: string;
  name: string;
  description?: string | null;
  icon_url?: string | null;
  member_count?: number;
  online_count?: number;
  owner_id?: string;
  [key: string]: unknown;
}

export type ChannelKind = 'text' | 'voice';

export interface Channel {
  id: string;
  name: string;
  kind: ChannelKind;
  community_id: string;
  position?: number;
  [key: string]: unknown;
}

export interface CommunityMember {
  id: string;
  username?: string;
  display_name?: string;
  status?: PresenceStatus;
  roles?: string[];
  [key: string]: unknown;
}

export interface MessageAuthor {
  id: string;
  username?: string;
  display_name?: string;
  [key: string]: unknown;
}

export interface ChatMessage {
  id: string;
  channel_id?: string;
  sender_id?: string;
  author?: MessageAuthor;
  content: string;
  created_at?: string;
  pending?: boolean;
  failed?: boolean;
  [key: string]: unknown;
}

export type PresenceStatus = 'online' | 'idle' | 'dnd' | 'offline';

export interface PresenceEntry {
  status: PresenceStatus;
  activity?: string | null;
  updated_at?: number;
}

export type PresenceMap = Record<string, PresenceEntry>;

export interface VoiceRoom {
  id: string;
  name: string;
  community_id?: string;
  participant_count?: number;
  max_participants?: number;
  [key: string]: unknown;
}

export interface VoiceParticipant {
  id: string;
  [key: string]: unknown;
}

/** Loosely-typed inbound/outbound WebSocket frame — the `type` field is the discriminant. */
export interface WsFrame {
  type: string;
  [key: string]: unknown;
}

export interface ActionResult {
  success: boolean;
  error?: string;
  [key: string]: unknown;
}
