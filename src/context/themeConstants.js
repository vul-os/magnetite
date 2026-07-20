/*
 * Theme registry.
 *
 * NOTE — this file used to hold the actual colour values, which ThemeContext
 * then wrote onto <html> as INLINE STYLES. That silently defeated the entire
 * design system: inline styles outrank stylesheet rules, so the palette in
 * src/styles/tokens.css never reached the page and every screen rendered in a
 * flat neutral ramp instead. The colours have been removed and tokens.css is
 * now the single authority; ThemeContext only sets `data-theme` on <html>.
 *
 * Do not reintroduce colour values here.
 */

/** The themes a user can select. `system` follows the OS preference. */
export const THEME_NAMES = ['dark', 'light', 'system'];

/** Backwards-compatible shape: callers use `Object.keys(themes)` for the list. */
export const themes = {
  dark: {},
  light: {},
  system: {},
};
