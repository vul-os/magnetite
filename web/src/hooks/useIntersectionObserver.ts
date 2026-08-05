import { useState, useEffect, useRef, useCallback } from 'react';

export function useIntersectionObserver(options: IntersectionObserverInit = {}) {
  const [isIntersecting, setIsIntersecting] = useState(false);
  const ref = useRef<Element | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        setIsIntersecting(entry.isIntersecting);
      },
      { threshold: 0.1, ...options }
    );

    observer.observe(element);

    return () => {
      observer.disconnect();
    };
  }, [options]);

  const setRef = useCallback((node: Element | null) => {
    ref.current = node;
  }, []);

  return { ref: setRef, isIntersecting };
}
