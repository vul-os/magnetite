/**
 * Shared lightweight domain types used by several hooks/components outside
 * the comms feature (see types/comms.ts for that cluster). These mirror the
 * backend's JSON shapes loosely — fields the frontend does not consume are
 * captured by the index signature rather than enumerated.
 */

export interface Game {
  id: string | number;
  title: string;
  developer?: string;
  description?: string;
  fee_per_session?: number;
  thumbnail?: string;
  players_online?: number;
  rating?: number;
  category?: string;
  genre?: string;
  tags?: string[];
  [key: string]: unknown;
}

export interface LeaderboardEntry {
  rank: number;
  username: string;
  score: number;
  [key: string]: unknown;
}

/** The authenticated user, as returned by /auth/me, /auth/login, /auth/register. */
export interface AuthUser {
  id: string;
  username?: string;
  email?: string;
  wallet_address?: string;
  created_at?: string;
  [key: string]: unknown;
}

export interface SearchResultItem {
  type: 'game' | 'user';
  id: string | number;
  title: string;
  subtitle: string;
  [key: string]: unknown;
}

export interface SearchResults {
  games: SearchResultItem[];
  users: SearchResultItem[];
}

export interface ReviewUser {
  username?: string;
  avatar?: string;
  [key: string]: unknown;
}

export interface Review {
  id?: string | number;
  user?: ReviewUser;
  rating: number;
  content?: string;
  date?: string;
  helpful?: number;
  [key: string]: unknown;
}
