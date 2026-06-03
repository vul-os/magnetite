import { useState, useEffect } from 'react';

export function useTypingEffect(text, speed = 50) {
  const [displayedText, setDisplayedText] = useState('');

  // Drives a typewriter animation via setInterval (external timer); resetting the
  // displayed text when the source text changes is part of that synchronization.
  useEffect(() => {
    if (!text) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setDisplayedText('');
      return;
    }

    let currentIndex = 0;
    setDisplayedText('');

    const interval = setInterval(() => {
      if (currentIndex < text.length) {
        setDisplayedText(text.slice(0, currentIndex + 1));
        currentIndex++;
      } else {
        clearInterval(interval);
      }
    }, speed);

    return () => clearInterval(interval);
  }, [text, speed]);

  return displayedText;
}
