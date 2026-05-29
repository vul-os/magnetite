import { useEffect, useRef, useCallback } from 'react';

export function useKeyboardShortcuts(shortcuts, enabled = true) {
  const pendingSequence = useRef(null);
  const sequenceTimeout = useRef(null);

  const handleKeyDown = useCallback((event) => {
    if (!enabled) return;

    const key = event.key;
    const pending = pendingSequence.current;

    if (pending) {
      const fullKey = `${pending} ${key}`;
      const shortcut = shortcuts.find(s => s.key.toLowerCase() === fullKey.toLowerCase());

      if (shortcut) {
        event.preventDefault();
        shortcut.action();
        pendingSequence.current = null;
        clearTimeout(sequenceTimeout.current);
        return;
      }

      const singleShortcut = shortcuts.find(s => s.key.toLowerCase() === pending.toLowerCase());
      if (singleShortcut && key.length === 1) {
        pendingSequence.current = null;
        clearTimeout(sequenceTimeout.current);
      }
    }

    const sequenceStart = shortcuts.find(s => {
      const parts = s.key.split(' ');
      return parts.length === 2 && parts[0].toLowerCase() === key.toLowerCase();
    });

    if (sequenceStart) {
      event.preventDefault();
      pendingSequence.current = key;
      clearTimeout(sequenceTimeout.current);
      sequenceTimeout.current = setTimeout(() => {
        pendingSequence.current = null;
      }, 500);
      return;
    }

    const shortcut = shortcuts.find(s => {
      const parts = s.key.split(' ');
      return parts.length === 1 && s.key.toLowerCase() === key.toLowerCase();
    });

    if (shortcut) {
      const target = event.target;
      const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;
      if (!isInput || shortcut.allowInInput) {
        event.preventDefault();
        shortcut.action();
      }
    }
  }, [shortcuts, enabled]);

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      clearTimeout(sequenceTimeout.current);
    };
  }, [handleKeyDown]);
}