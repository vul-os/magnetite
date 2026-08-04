import { useState, useEffect } from 'react';

export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(false);

  // Subscribes to a matchMedia list (external system) and seeds the initial
  // match value, which can only be read synchronously inside the effect.
  useEffect(() => {
    const mq = window.matchMedia(query);
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setMatches(mq.matches);

    function handler(e: MediaQueryListEvent) {
      setMatches(e.matches);
    }

    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [query]);

  return matches;
}
