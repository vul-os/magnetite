import { useState, useEffect, useCallback, useRef } from 'react';

// Derive the WebSocket base URL from the API base env var.
// VITE_API_URL may be http(s)://... — convert to ws(s)://.
function getWsBase() {
  const apiBase = import.meta.env.VITE_API_URL || 'http://localhost:8080';
  return apiBase.replace(/^http/, 'ws');
}

export function useWebSocket(url, options = {}) {
  const { autoReconnect = true, heartbeatInterval = 30000, reconnectDelay = 3000 } = options;

  const [socket, setSocket] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState(null);

  const messageQueueRef = useRef([]);
  const reconnectTimeoutRef = useRef(null);
  const heartbeatTimeoutRef = useRef(null);
  // Stable ref so onclose can call the latest `connect` without creating a circular dep
  const connectRef = useRef(null);

  const sendMessage = useCallback((data) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(typeof data === 'string' ? data : JSON.stringify(data));
    } else {
      messageQueueRef.current.push(data);
    }
  }, [socket]);

  const flushQueue = useCallback((ws) => {
    while (messageQueueRef.current.length > 0) {
      const msg = messageQueueRef.current.shift();
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(typeof msg === 'string' ? msg : JSON.stringify(msg));
      }
    }
  }, []);

  const startHeartbeat = useCallback((ws) => {
    if (heartbeatTimeoutRef.current) {
      clearInterval(heartbeatTimeoutRef.current);
    }
    heartbeatTimeoutRef.current = setInterval(() => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, heartbeatInterval);
  }, [heartbeatInterval]);

  const connect = useCallback(() => {
    // Resolve full WS URL
    let wsUrl;
    if (url.startsWith('ws://') || url.startsWith('wss://')) {
      wsUrl = url;
    } else {
      wsUrl = `${getWsBase()}${url}`;
    }

    // Keep a token-free copy of the URL for safe use in error logs.
    const wsUrlSafe = wsUrl;

    // Append ?token=<jwt> so every backend WS handler can authenticate the connection.
    // backend/src/ws/comms.rs, voice.rs, and game.rs all require validate_token(query.token).
    // The token is NEVER written to any log — only wsUrlSafe (without the token) is logged.
    const token = localStorage.getItem('token');
    if (token) {
      const sep = wsUrl.includes('?') ? '&' : '?';
      wsUrl = `${wsUrl}${sep}token=${encodeURIComponent(token)}`;
    }

    // Use a real browser WebSocket by default.
    // Only substitute the mock implementation when VITE_USE_MOCK_WS === 'true'
    // (local dev without a backend).
    let ws;
    if (import.meta.env.VITE_USE_MOCK_WS === 'true') {
      ws = createMockSocket(wsUrl);
    } else {
      try {
        ws = new WebSocket(wsUrl);
      } catch (err) {
        // Log the token-free URL only — never log the full wsUrl which contains the JWT.
        console.error('[useWebSocket] Failed to construct WebSocket:', wsUrlSafe, err);
        return null;
      }
    }

    ws.onopen = () => {
      setIsConnected(true);
      setSocket(ws);
      flushQueue(ws);
      startHeartbeat(ws);
    };

    ws.onclose = () => {
      setIsConnected(false);
      setSocket(null);
      if (heartbeatTimeoutRef.current) {
        clearInterval(heartbeatTimeoutRef.current);
      }
      if (autoReconnect) {
        reconnectTimeoutRef.current = setTimeout(() => connectRef.current?.(), reconnectDelay);
      }
    };

    ws.onerror = (event) => {
      // Log the token-free URL — wsUrl (which carries the JWT) is never logged.
      console.error('[useWebSocket] error on', wsUrlSafe, event);
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === 'pong') return;
        setLastMessage(data);
      } catch {
        setLastMessage(event.data);
      }
    };

    return ws;
  }, [url, autoReconnect, reconnectDelay, flushQueue, startHeartbeat]);

  // Keep the ref current so the onclose handler always calls the latest connect.
  // Use an effect so we don't write refs during render.
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  const reconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }
    connect();
  }, [connect]);

  useEffect(() => {
    const ws = connect();
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (heartbeatTimeoutRef.current) {
        clearInterval(heartbeatTimeoutRef.current);
      }
      if (ws) ws.close();
    };
  }, [connect]);

  return { socket, isConnected, lastMessage, sendMessage, reconnect };
}

// ---------------------------------------------------------------------------
// Mock socket — only used when VITE_USE_MOCK_WS === 'true'
// ---------------------------------------------------------------------------
function createMockSocket(url) {
  let readyState = 0;
  const listeners = {};

  const mockSocket = {
    url,
    get readyState() { return readyState; },

    send: (data) => {
      setTimeout(() => {
        const messageListeners = listeners['message'];
        if (messageListeners && messageListeners.length > 0) {
          let parsed;
          try {
            parsed = JSON.parse(data);
          } catch {
            parsed = data;
          }
          if (parsed && parsed.type === 'ping') {
            setTimeout(() => {
              const pl = listeners['message'];
              if (pl) {
                pl.forEach((l) =>
                  l({ data: JSON.stringify({ type: 'pong' }) })
                );
              }
            }, 100);
          }
        }
      }, 50);
    },

    close: () => {
      readyState = 3;
      const closeListeners = listeners['close'];
      if (closeListeners) {
        closeListeners.forEach((l) => l());
      }
    },

    set onopen(fn)    { listeners['open'] = [fn]; },
    set onclose(fn)   { listeners['close'] = [fn]; },
    set onmessage(fn) { listeners['message'] = [fn]; },
    set onerror(fn)   { listeners['error'] = [fn]; },

    addEventListener: (event, callback) => {
      if (!listeners[event]) listeners[event] = [];
      listeners[event].push(callback);
    },

    removeEventListener: (event, callback) => {
      if (listeners[event]) {
        listeners[event] = listeners[event].filter((l) => l !== callback);
      }
    },
  };

  setTimeout(() => {
    readyState = 1;
    const openListeners = listeners['open'];
    if (openListeners) {
      openListeners.forEach((l) => l());
    }
  }, 100);

  return mockSocket;
}
