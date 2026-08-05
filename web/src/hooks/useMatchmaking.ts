import { useState, useCallback } from 'react';
import { api } from '../api/client';

export type MatchmakingStatus = 'searching' | 'found' | 'error' | null;

export interface MatchData {
  status?: string;
  [key: string]: unknown;
}

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

export function useMatchmaking() {
  const [status, setStatus] = useState<MatchmakingStatus>(null);
  const [matchData, setMatchData] = useState<MatchData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const joinQueue = useCallback(async (gameId: string | number) => {
    try {
      setLoading(true);
      setError(null);
      setStatus('searching');
      const data = await api.matchmaking.join(gameId) as MatchData;
      setMatchData(data);
      setStatus((data?.status as MatchmakingStatus) || 'searching');
    } catch (err) {
      setError(errMessage(err, 'unknown error'));
      setStatus('error');
    } finally {
      setLoading(false);
    }
  }, []);

  const leaveQueue = useCallback(async () => {
    try {
      setLoading(true);
      await api.matchmaking.leave();
      setStatus(null);
      setMatchData(null);
    } catch {
      /* ignore leave errors */
    } finally {
      setLoading(false);
    }
  }, []);

  const pollStatus = useCallback(async () => {
    try {
      const data = await api.matchmaking.status() as MatchData;
      setMatchData(data);
      setStatus((data?.status as MatchmakingStatus) || status);
    } catch {
      /* ignore polling errors */
    }
  }, [status]);

  return { status, matchData, loading, error, joinQueue, leaveQueue, pollStatus };
}
