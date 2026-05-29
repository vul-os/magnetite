import { useCallback } from 'react';

export function useAnimation() {
  const animate = useCallback((element, keyframes, options = {}) => {
    if (!element) return null;
    return element.animate(keyframes, { duration: 300, fill: 'forwards', ...options });
  }, []);

  return { animate };
}
