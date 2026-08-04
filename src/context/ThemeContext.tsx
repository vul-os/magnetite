import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { themes, type ThemeName } from './themeConstants';

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

export interface ThemeContextValue {
  theme: ThemeName;
  setTheme: (theme: ThemeName) => void;
  themes: string[];
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

const STORAGE_KEY = 'theme';

/** Resolve a stored preference to the theme that should actually be applied. */
function resolve(pref: ThemeName): ThemeName {
  if (pref !== 'system') return pref;
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<ThemeName>(() => {
    try {
      return (localStorage.getItem(STORAGE_KEY) as ThemeName) || 'dark';
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
