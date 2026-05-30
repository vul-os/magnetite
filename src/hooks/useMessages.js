import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api/client';

// ── Mock fallback data ─────────────────────────────────────────────────────
function makeMockMessages(channelId) {
  const now = Date.now();
  return [
    {
      id: `m-${channelId}-1`,
      channel_id: channelId,
      author: { id: '2', username: 'player_two', display_name: 'Player Two' },
      content: 'Hey everyone — anyone building a Bevy game right now?',
      created_at: new Date(now - 300_000).toISOString(),
    },
    {
      id: `m-${channelId}-2`,
      channel_id: channelId,
      author: { id: '1', username: 'dev_one', display_name: 'Dev One' },
      content: 'Yeah! Working on a top-down shooter with rapier physics. The SDK integration is solid.',
      created_at: new Date(now - 240_000).toISOString(),
    },
    {
      id: `m-${channelId}-3`,
      channel_id: channelId,
      author: { id: '3', username: 'streamer', display_name: 'StreamerX' },
      content: 'Going live soon — motorsport prototype with controller support. Check the streams tab!',
      created_at: new Date(now - 60_000).toISOString(),
    },
  ];
}

// ── Hook ───────────────────────────────────────────────────────────────────
export function useMessages(channelId, { isDM = false, dmUserId = null } = {}) {
  const [messages, setMessages] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [hasMore, setHasMore] = useState(true);
  const oldestIdRef = useRef(null);

  // Reset when channel/DM target changes
  useEffect(() => {
    if (!channelId && !isDM) return;
    setMessages([]);
    oldestIdRef.current = null;
    setHasMore(true);
  }, [channelId, isDM, dmUserId]);

  const fetchMessages = useCallback(async (params = {}) => {
    let cancelled = false;
    try {
      setLoading(true);
      setError(null);

      let data;
      if (isDM && dmUserId) {
        data = await api.messages.listDMs(dmUserId, params);
      } else if (channelId) {
        data = await api.messages.list(channelId, params);
      } else {
        return;
      }

      const fetched = Array.isArray(data) ? data : (data?.messages ?? makeMockMessages(channelId ?? dmUserId));
      if (!cancelled) {
        if (params.before) {
          setMessages((prev) => [...fetched, ...prev]);
        } else {
          setMessages(fetched);
        }
        if (fetched.length > 0) {
          oldestIdRef.current = fetched[0]?.id ?? null;
        }
        setHasMore(fetched.length >= (params.limit ?? 50));
      }
    } catch {
      if (!cancelled) {
        const mocks = makeMockMessages(channelId ?? dmUserId ?? 'dm');
        if (params.before) {
          setMessages((prev) => [...mocks, ...prev]);
        } else {
          setMessages(mocks);
        }
        setHasMore(false);
      }
    } finally {
      if (!cancelled) setLoading(false);
    }
    return () => { cancelled = true; };
  }, [channelId, isDM, dmUserId]);

  useEffect(() => {
    if (channelId || (isDM && dmUserId)) {
      fetchMessages({ limit: 50 });
    }
  }, [fetchMessages, channelId, isDM, dmUserId]);

  /** Load older messages (pagination — prepend). */
  const loadMore = useCallback(() => {
    if (!hasMore || loading || !oldestIdRef.current) return;
    fetchMessages({ limit: 50, before: oldestIdRef.current });
  }, [hasMore, loading, fetchMessages]);

  /** Append a locally-sent message optimistically. */
  const appendMessage = useCallback((message) => {
    setMessages((prev) => [...prev, message]);
  }, []);

  /** Post a new message; appends optimistically before the server round-trip. */
  const postMessage = useCallback(async (content) => {
    const tempId = `temp-${Date.now()}`;
    const tempMsg = {
      id: tempId,
      channel_id: channelId,
      author: { id: 'me', username: 'you', display_name: 'You' },
      content,
      created_at: new Date().toISOString(),
      pending: true,
    };
    appendMessage(tempMsg);

    try {
      let confirmed;
      if (isDM && dmUserId) {
        confirmed = await api.messages.sendDM(dmUserId, { content });
      } else {
        confirmed = await api.messages.post(channelId, { content });
      }
      // Replace temp message with confirmed
      setMessages((prev) =>
        prev.map((m) => (m.id === tempId ? { ...confirmed, pending: false } : m))
      );
      return { success: true, message: confirmed };
    } catch (err) {
      // Mark as failed but keep it visible
      setMessages((prev) =>
        prev.map((m) => (m.id === tempId ? { ...m, pending: false, failed: true } : m))
      );
      return { success: false, error: err.message };
    }
  }, [channelId, isDM, dmUserId, appendMessage]);

  return {
    messages,
    loading,
    error,
    hasMore,
    loadMore,
    postMessage,
    appendMessage,
    setMessages,
  };
}
