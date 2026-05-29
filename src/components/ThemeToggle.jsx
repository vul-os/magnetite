import { useTheme } from '../hooks/useTheme';
import './ThemeToggle.css';

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  const toggleTheme = () => {
    const themeOrder = ['dark', 'light', 'system'];
    const currentIndex = themeOrder.indexOf(theme);
    const nextIndex = (currentIndex + 1) % themeOrder.length;
    setTheme(themeOrder[nextIndex]);
  };

  const getIcon = () => {
    if (theme === 'light') return '☀️';
    if (theme === 'system') return '💻';
    return '🌙';
  };

  const getLabel = () => {
    return theme.charAt(0).toUpperCase() + theme.slice(1);
  };

  return (
    <button
      className="theme-toggle"
      onClick={toggleTheme}
      aria-label={`Current theme: ${theme}. Click to change.`}
      title={`Theme: ${getLabel()}`}
    >
      <span className="theme-icon">{getIcon()}</span>
      <span className="theme-label">{getLabel()}</span>
    </button>
  );
}