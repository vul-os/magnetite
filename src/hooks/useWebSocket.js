import { useState, useEffect, useCallback, useRef } from 'react';

export function useWebSocket(url, options = {}) {
  const { autoReconnect = true, heartbeatInterval = 30000, reconnectDelay = 3000 } = options;

  const [socket, setSocket] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState(null);

  const messageQueueRef = useRef([]);
  const reconnectTimeoutRef = useRef(null);
  const heartbeatTimeoutRef = useRef(null);

  const sendMessage = useCallback((data) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(typeof data === 'string' ? data : JSON.stringify(data));
    } else {
      messageQueueRef.current.push(data);
    }
  }, [socket]);

  const flushQueue = useCallback(() => {
    while (messageQueueRef.current.length > 0) {
      const msg = messageQueueRef.current.shift();
      sendMessage(msg);
    }
  }, [sendMessage]);

  const startHeartbeat = useCallback(() => {
    if (heartbeatTimeoutRef.current) {
      clearInterval(heartbeatTimeoutRef.current);
    }
    heartbeatTimeoutRef.current = setInterval(() => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'ping' }));
      }
    }, heartbeatInterval);
  }, [socket, heartbeatInterval]);

  const connect = useCallback(() => {
    const wsUrl = url.startsWith('ws') ? url : `ws://localhost:8080${url}`;
    const mockSocket = createMockSocket(wsUrl);

    mockSocket.onopen = () => {
      setIsConnected(true);
      setSocket(mockSocket);
      flushQueue();
      startHeartbeat();
    };

    mockSocket.onclose = () => {
      setIsConnected(false);
      setSocket(null);
      if (heartbeatTimeoutRef.current) {
        clearInterval(heartbeatTimeoutRef.current);
      }
      if (autoReconnect) {
        reconnectTimeoutRef.current = setTimeout(connect, reconnectDelay);
      }
    };

    mockSocket.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === 'pong') return;
        setLastMessage(data);
      } catch {
        setLastMessage(event.data);
      }
    };

    return mockSocket;
  }, [url, autoReconnect, reconnectDelay, flushQueue, startHeartbeat]);

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
      ws.close();
    };
  }, [connect]);

  return { socket, isConnected, lastMessage, sendMessage, reconnect };
}

function createMockSocket(url) {
  let readyState = 0;
  const listeners = {};

  const mockSocket = {
    url,
    readyState,

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
          if (parsed.type === 'ping') {
            const pongListeners = listeners['message'];
            setTimeout(() => {
              if (pongListeners) {
                pongListeners.forEach((l) =>
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

    addEventListener: (event, callback) => {
      if (!listeners[event]) {
        listeners[event] = [];
      }
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
