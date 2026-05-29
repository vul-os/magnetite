import { createContext, useContext, useState, useEffect } from 'react';
import { themes } from './themeConstants';

const ThemeContext = createContext();

export function ThemeProvider({ children }) {
  const [theme, setTheme] = useState(() => {
    const stored = localStorage.getItem('theme');
    return stored || 'dark';
  });

  useEffect(() => {
    localStorage.setItem('theme', theme);
    
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const applySystemTheme = () => {
        const isDark = mediaQuery.matches;
        const root = document.documentElement;
        Object.entries(themes[isDark ? 'dark' : 'light']).forEach(([key, value]) => {
          root.style.setProperty(key, value);
        });
        root.setAttribute('data-theme', isDark ? 'dark' : 'light');
      };
      applySystemTheme();
      mediaQuery.addEventListener('change', applySystemTheme);
      return () => mediaQuery.removeEventListener('change', applySystemTheme);
    } else {
      const root = document.documentElement;
      Object.entries(themes[theme]).forEach(([key, value]) => {
        root.style.setProperty(key, value);
      });
      root.setAttribute('data-theme', theme);
    }
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme, themes: Object.keys(themes) }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error('useTheme must be used within ThemeProvider');
  return context;
}

export { themes };