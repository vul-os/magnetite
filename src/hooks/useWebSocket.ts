import { useState, useEffect, useCallback, useRef } from 'react';
import type { WsFrame } from '../types/comms';

export interface UseWebSocketOptions {
  autoReconnect?: boolean;
  heartbeatInterval?: number;
  reconnectDelay?: number;
}

/**
 * The minimal WebSocket-shaped surface this hook relies on. A real
 * `WebSocket` satisfies this; so does the duck-typed dev-mode mock socket
 * below (see `createMockSocket`).
 */
export interface WsLike {
  readonly url: string;
  readonly readyState: number;
  send(data: string): void;
  close(): void;
  onopen: (() => void) | null;
  onclose: (() => void) | null;
  onmessage: ((event: { data: string }) => void) | null;
  onerror: ((event: unknown) => void) | null;
  addEventListener(event: string, callback: (...args: unknown[]) => void): void;
  removeEventListener(event: string, callback: (...args: unknown[]) => void): void;
}

// Derive the WebSocket base URL from the API base env var.
// VITE_API_URL may be http(s)://... — convert to ws(s)://.
function getWsBase(): string {
  const apiBase = import.meta.env.VITE_API_URL || 'http://localhost:8080';
  return apiBase.replace(/^http/, 'ws');
}

export function useWebSocket(url: string, options: UseWebSocketOptions = {}) {
  const { autoReconnect = true, heartbeatInterval = 30000, reconnectDelay = 3000 } = options;

  const [socket, setSocket] = useState<WsLike | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState<WsFrame | string | null>(null);

  const messageQueueRef = useRef<(string | WsFrame)[]>([]);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const heartbeatTimeoutRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Stable ref so onclose can call the latest `connect` without creating a circular dep
  const connectRef = useRef<(() => WsLike | null) | null>(null);

  const sendMessage = useCallback((data: string | WsFrame) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(typeof data === 'string' ? data : JSON.stringify(data));
    } else {
      messageQueueRef.current.push(data);
    }
  }, [socket]);

  const flushQueue = useCallback((ws: WsLike | null) => {
    while (messageQueueRef.current.length > 0) {
      const msg = messageQueueRef.current.shift();
      if (ws && ws.readyState === WebSocket.OPEN && msg !== undefined) {
        ws.send(typeof msg === 'string' ? msg : JSON.stringify(msg));
      }
    }
  }, []);

  const startHeartbeat = useCallback((ws: WsLike | null) => {
    if (heartbeatTimeoutRef.current) {
      clearInterval(heartbeatTimeoutRef.current);
    }
    heartbeatTimeoutRef.current = setInterval(() => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, heartbeatInterval);
  }, [heartbeatInterval]);

  const connect = useCallback((): WsLike | null => {
    // Resolve full WS URL
    let wsUrl: string;
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
    let ws: WsLike;
    if (import.meta.env.VITE_USE_MOCK_WS === 'true') {
      ws = createMockSocket(wsUrl);
    } else {
      try {
        // A real WebSocket structurally satisfies WsLike.
        ws = new WebSocket(wsUrl) as unknown as WsLike;
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
        const data = JSON.parse(event.data) as WsFrame;
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
type MockListener = (...args: unknown[]) => void;

function createMockSocket(url: string): WsLike {
  let readyState = 0;
  const listeners: Record<string, MockListener[]> = {};

  const mockSocket: WsLike = {
    url,
    get readyState() { return readyState; },

    send: (data: string) => {
      setTimeout(() => {
        const messageListeners = listeners['message'];
        if (messageListeners && messageListeners.length > 0) {
          let parsed: unknown;
          try {
            parsed = JSON.parse(data);
          } catch {
            parsed = data;
          }
          if (parsed && typeof parsed === 'object' && (parsed as WsFrame).type === 'ping') {
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

    set onopen(fn: (() => void) | null) { listeners['open'] = fn ? [fn] : []; },
    set onclose(fn: (() => void) | null) { listeners['close'] = fn ? [fn] : []; },
    set onmessage(fn: ((event: { data: string }) => void) | null) { listeners['message'] = fn ? [fn as MockListener] : []; },
    set onerror(fn: ((event: unknown) => void) | null) { listeners['error'] = fn ? [fn] : []; },

    addEventListener: (event: string, callback: MockListener) => {
      if (!listeners[event]) listeners[event] = [];
      listeners[event].push(callback);
    },

    removeEventListener: (event: string, callback: MockListener) => {
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
