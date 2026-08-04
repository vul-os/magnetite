import { useEffect, useRef, useCallback } from 'react';

export function useFocusTrap(isActive = true) {
  const containerRef = useRef<HTMLElement | null>(null);
  const previousActiveElement = useRef<Element | null>(null);

  const getFocusableElements = useCallback((container: HTMLElement | null): HTMLElement[] => {
    if (!container) return [];

    const focusableSelectors = [
      'button:not([disabled])',
      'a[href]',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(', ');

    return Array.from(container.querySelectorAll<HTMLElement>(focusableSelectors));
  }, []);

  useEffect(() => {
    if (!isActive || !containerRef.current) return;

    previousActiveElement.current = document.activeElement;

    const container = containerRef.current;
    const focusableElements = getFocusableElements(container);
    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;

      const activeElement = document.activeElement;

      if (e.shiftKey) {
        if (activeElement === firstElement || !container.contains(activeElement)) {
          e.preventDefault();
          lastElement?.focus();
        }
      } else {
        if (activeElement === lastElement || !container.contains(activeElement)) {
          e.preventDefault();
          firstElement?.focus();
        }
      }
    };

    container.addEventListener('keydown', handleKeyDown);
    firstElement?.focus();

    return () => {
      container.removeEventListener('keydown', handleKeyDown);
      const prev = previousActiveElement.current as HTMLElement | null;
      if (prev && prev.focus) {
        prev.focus();
      }
    };
  }, [isActive, getFocusableElements]);

  return containerRef;
}
