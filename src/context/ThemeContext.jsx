import { createContext, useContext, useState, useEffect } from 'react';
import { themes } from './themeConstants';

/*
 * Theme provider.
 *
 * Responsibility is deliberately narrow: resolve the user's choice
 * ('dark' | 'light' | 'system') to a concrete theme and publish it as
 * `data-theme` on <html>. All colour lives in src/styles/tokens.css, which
 * keys off that attribute.
 *
 * This provider must never write colour values as inline styles — inline
 * styles beat stylesheet rules and would override the token layer.
 */

const ThemeContext = createContext();

const STORAGE_KEY = 'theme';

/** Resolve a stored preference to the theme that should actually be applied. */
function resolve(pref) {
  if (pref !== 'system') return pref;
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function ThemeProvider({ children }) {
  const [theme, setTheme] = useState(() => {
    try {
      return localStorage.getItem(STORAGE_KEY) || 'dark';
    } catch {
      // Private mode / storage disabled — fall back to the default.
      return 'dark';
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // Persistence is best-effort; the theme still applies for this session.
    }

    const root = document.documentElement;
    const apply = () => root.setAttribute('data-theme', resolve(theme));

    apply();

    // Only 'system' needs to react to OS changes.
    if (theme !== 'system') return;

    const mq = window.matchMedia?.('(prefers-color-scheme: dark)');
    if (!mq) return;
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme, themes: Object.keys(themes) }}>
      {children}
    </ThemeContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error('useTheme must be used within ThemeProvider');
  return context;
}

// Re-export of the theme constants for convenience.
// eslint-disable-next-line react-refresh/only-export-components
export { themes };
