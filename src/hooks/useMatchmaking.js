import { useState, useCallback } from 'react';
import { api } from '../api/client';

export function useMatchmaking() {
  const [status, setStatus] = useState(null); // null | 'searching' | 'found' | 'error'
  const [matchData, setMatchData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const joinQueue = useCallback(async (gameId) => {
    try {
      setLoading(true);
      setError(null);
      setStatus('searching');
      const data = await api.matchmaking.join(gameId);
      setMatchData(data);
      setStatus(data?.status || 'searching');
    } catch (err) {
      setError(err.message);
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
      const data = await api.matchmaking.status();
      setMatchData(data);
      setStatus(data?.status || status);
    } catch {
      /* ignore polling errors */
    }
  }, [status]);

  return { status, matchData, loading, error, joinQueue, leaveQueue, pollStatus };
}
