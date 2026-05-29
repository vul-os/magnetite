import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import './BuildLogs.css';

const ANSI_COLORS = {
  black: '#000',
  red: '#ef4444',
  green: '#22c55e',
  yellow: '#eab308',
  blue: '#3b82f6',
  magenta: '#a855f7',
  cyan: '#06b6d4',
  white: '#fff',
  brightBlack: '#6b7280',
  brightRed: '#f87171',
  brightGreen: '#4ade80',
  brightYellow: '#facc15',
  brightBlue: '#60a5fa',
  brightMagenta: '#c084fc',
  brightCyan: '#22d3ee',
  brightWhite: '#f9fafb',
};

const parseAnsi = (text) => {
  const parts = [];
  const ESC = '\x1b';
  const regex = new RegExp(`${ESC}\\[([0-9;]*)m`, 'g');
  let lastIndex = 0;
  let currentColor = null;

  const processSequence = (sequence) => {
    if (!sequence) {
      currentColor = null;
      return;
    }

    const codes = sequence.split(';').map(Number);
    for (const code of codes) {
      if (code === 0) {
        currentColor = null;
      } else if (code === 1) {
        currentColor = currentColor ? currentColor.replace('color:', 'color:bright-') : null;
      } else if (code >= 30 && code <= 37) {
        const colorName = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'][code - 30];
        currentColor = `color:${colorName}`;
      } else if (code >= 90 && code <= 97) {
        const colorName = ['brightBlack', 'brightRed', 'brightGreen', 'brightYellow', 'brightBlue', 'brightMagenta', 'brightCyan', 'brightWhite'][code - 90];
        currentColor = `color:${colorName}`;
      }
    }
  };

  let match;
  while ((match = regex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      const content = text.slice(lastIndex, match.index);
      parts.push({ content, color: currentColor });
    }
    processSequence(match[1]);
    lastIndex = regex.lastIndex;
  }

  if (lastIndex < text.length) {
    parts.push({ content: text.slice(lastIndex), color: currentColor });
  }

  return parts;
};

const getStyle = (color) => {
  if (!color) return {};
  const [, colorName] = color.split(':');
  return { color: ANSI_COLORS[colorName] || ANSI_COLORS.white };
};

export default function BuildLogs({ logs = '', isBuilding = false, onClear }) {
  const [searchTerm, setSearchTerm] = useState('');
  const [autoScroll, setAutoScroll] = useState(true);
  const logsEndRef = useRef(null);
  const logsContainerRef = useRef(null);

  const filteredLogs = useMemo(() => {
    if (!searchTerm.trim()) {
      return logs.split('\n');
    }
    const lines = logs.split('\n');
    return lines.filter(line =>
      line.toLowerCase().includes(searchTerm.toLowerCase())
    );
  }, [logs, searchTerm]);

  const scrollToBottom = useCallback(() => {
    if (autoScroll && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [autoScroll]);

  useEffect(() => {
    scrollToBottom();
  }, [logs, scrollToBottom]);

  const handleScroll = () => {
    if (!logsContainerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = logsContainerRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
    setAutoScroll(isAtBottom);
  };

  const highlightSearch = (line) => {
    if (!searchTerm.trim()) return line;

    const parts = [];
    const lowerLine = line.toLowerCase();
    const lowerSearch = searchTerm.toLowerCase();
    let lastIndex = 0;
    let index = lowerLine.indexOf(lowerSearch);

    while (index !== -1) {
      if (index > lastIndex) {
        parts.push({ text: line.slice(lastIndex, index), highlight: false });
      }
      parts.push({ text: line.slice(index, index + searchTerm.length), highlight: true });
      lastIndex = index + searchTerm.length;
      index = lowerLine.indexOf(lowerSearch, lastIndex);
    }

    if (lastIndex < line.length) {
      parts.push({ text: line.slice(lastIndex), highlight: false });
    }

    return parts;
  };

  const renderLine = (line, index) => {
    const parsed = parseAnsi(line);
    const highlighted = highlightSearch(line);

    return (
      <div key={index} className="build-log-line">
        <span className="log-line-number">{index + 1}</span>
        <span className="log-line-content">
          {parsed.length === 1 && !searchTerm.trim() ? (
            <span style={getStyle(parsed[0].color)}>{parsed[0].content}</span>
          ) : (
            Array.isArray(highlighted) ? (
              highlighted.map((part, i) =>
                part.highlight ? (
                  <mark key={i} className="log-highlight">{part.text}</mark>
                ) : (
                  <span key={i} style={getStyle(parsed.find((_, j) => j < i)?.color)}>{part.text}</span>
                )
              )
            ) : (
              <span style={getStyle(parsed[0]?.color)}>{highlighted}</span>
            )
          )}
        </span>
      </div>
    );
  };

  return (
    <div className="build-logs">
      <div className="build-logs-header">
        <div className="logs-title">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
            <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
          </svg>
          <span>Build Logs</span>
          {isBuilding && <span className="building-indicator" />}
        </div>

        <div className="logs-controls">
          <div className="logs-search">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" />
              <path d="M21 21l-4.35-4.35" />
            </svg>
            <input
              type="text"
              placeholder="Search logs..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            {searchTerm && (
              <button className="search-clear" onClick={() => setSearchTerm('')}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            )}
          </div>

          <button
            className={`auto-scroll-btn ${autoScroll ? 'active' : ''}`}
            onClick={() => setAutoScroll(!autoScroll)}
            title={autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll disabled'}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 5v14M5 12l7 7 7-7" />
            </svg>
          </button>

          {onClear && (
            <button className="clear-logs-btn" onClick={onClear} title="Clear logs">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2" />
              </svg>
            </button>
          )}
        </div>
      </div>

      <div
        className="build-logs-content"
        ref={logsContainerRef}
        onScroll={handleScroll}
      >
        {filteredLogs.length === 0 ? (
          <div className="logs-empty">
            {searchTerm ? 'No matching logs found' : 'Waiting for logs...'}
          </div>
        ) : (
          filteredLogs.map((line, index) => renderLine(line, index))
        )}
        <div ref={logsEndRef} />
      </div>

      <div className="build-logs-footer">
        <span className="logs-count">
          {filteredLogs.length} lines
          {searchTerm && ` (${logs.split('\n').filter(l => l.toLowerCase().includes(searchTerm.toLowerCase())).length} matches)`}
        </span>
        {isBuilding && <span className="building-status">Building...</span>}
      </div>
    </div>
  );
}
