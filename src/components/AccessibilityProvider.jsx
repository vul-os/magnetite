import { createContext, useContext, useState, useCallback, useRef, useEffect } from 'react';

const AccessibilityContext = createContext();

export function AccessibilityProvider({ children }) {
  const [focusTrapEnabled, setFocusTrapEnabled] = useState(false);
  const [announcements, setAnnouncements] = useState([]);
  const announcerRef = useRef(null);
  const previousActiveElement = useRef(null);

  const announce = useCallback((message, priority = 'polite') => {
    const id = Date.now();
    setAnnouncements(prev => [...prev, { id, message, priority }]);
    setTimeout(() => {
      setAnnouncements(prev => prev.filter(a => a.id !== id));
    }, 1000);
  }, []);

  const trapFocus = useCallback((containerRef) => {
    if (!containerRef?.current) return;

    const focusableSelectors = [
      'button:not([disabled])',
      'a[href]',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(', ');

    const focusableElements = containerRef.current.querySelectorAll(focusableSelectors);
    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];

    previousActiveElement.current = document.activeElement;

    const handleTabKey = (e) => {
      if (e.key !== 'Tab') return;

      if (e.shiftKey) {
        if (document.activeElement === firstElement) {
          e.preventDefault();
          lastElement?.focus();
        }
      } else {
        if (document.activeElement === lastElement) {
          e.preventDefault();
          firstElement?.focus();
        }
      }
    };

    containerRef.current.addEventListener('keydown', handleTabKey);
    firstElement?.focus();

    return () => {
      containerRef.current?.removeEventListener('keydown', handleTabKey);
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
      }
    };
  }, []);

  const releaseFocus = useCallback(() => {
    if (previousActiveElement.current) {
      previousActiveElement.current.focus();
      previousActiveElement.current = null;
    }
  }, []);

  const disableFocusTrap = useCallback(() => {
    setFocusTrapEnabled(false);
  }, []);

  const enableFocusTrap = useCallback(() => {
    setFocusTrapEnabled(true);
  }, []);

  useEffect(() => {
    if (!announcerRef.current) return;

    const politeAnnouncements = announcements.filter(a => a.priority === 'polite');
    const assertiveAnnouncements = announcements.filter(a => a.priority === 'assertive');

    politeAnnouncements.forEach(a => {
      announcerRef.current.setAttribute('aria-live', 'polite');
      announcerRef.current.textContent = a.message;
    });

    assertiveAnnouncements.forEach(a => {
      announcerRef.current.setAttribute('aria-live', 'assertive');
      announcerRef.current.textContent = a.message;
    });
  }, [announcements]);

  const value = {
    announce,
    trapFocus,
    releaseFocus,
    enableFocusTrap,
    disableFocusTrap,
    focusTrapEnabled,
  };

  return (
    <AccessibilityContext.Provider value={value}>
      <div
        ref={announcerRef}
        role="status"
        aria-live="polite"
        aria-atomic="true"
        style={{
          position: 'absolute',
          width: '1px',
          height: '1px',
          padding: 0,
          margin: '-1px',
          overflow: 'hidden',
          clip: 'rect(0, 0, 0, 0)',
          whiteSpace: 'nowrap',
          border: 0,
        }}
      />
      {children}
    </AccessibilityContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated; this hook is stable
// and does not affect fast-refresh of the component in practice.
// eslint-disable-next-line react-refresh/only-export-components
export function useAccessibility() {
  const context = useContext(AccessibilityContext);
  if (!context) {
    throw new Error('useAccessibility must be used within AccessibilityProvider');
  }
  return context;
}
