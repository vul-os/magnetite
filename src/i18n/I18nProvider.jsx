/**
 * I18nProvider — lightweight React i18n context provider.
 *
 * Loads locale JSON files lazily and exposes them via I18nContext.
 * The English locale ("en") is the default and always loaded eagerly as
 * the fallback dictionary.
 *
 * This is a SCAFFOLD — it does NOT rewire existing pages.  New pages and
 * future migrations should wrap the component tree with <I18nProvider> and
 * use the useTranslation() hook.
 *
 * Usage:
 *   // In main.jsx (optional — wrap the app when ready to enable i18n):
 *   import { I18nProvider } from './i18n/I18nProvider';
 *   <I18nProvider><App /></I18nProvider>
 */

import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from 'react';
import enMessages from './en.json';

/** @type {React.Context<import('./useTranslation').I18nContextValue|null>} */
export const I18nContext = createContext(null);

/**
 * Detect the browser's preferred locale, clamped to supported locales.
 *
 * @param {string[]} supported  List of supported locale codes.
 * @returns {string}  The best matching locale, defaulting to "en".
 */
function detectLocale(supported) {
  const preferred = navigator?.language ?? 'en';
  const lang = preferred.split('-')[0].toLowerCase();
  return supported.includes(lang) ? lang : 'en';
}

/**
 * Dynamically import a locale JSON file.  Falls back to `enMessages` if the
 * locale is "en" or if the import fails.
 *
 * @param {string} locale
 * @returns {Promise<object>}  Resolves to the translation dictionary.
 */
async function loadLocale(locale) {
  if (locale === 'en') return enMessages;
  try {
    // Dynamic imports for additional locales — add entries here as new
    // locale JSON files are created under src/i18n/<locale>.json.
    const modules = {
      // 'fr': () => import('./fr.json'),
      // 'de': () => import('./de.json'),
    };
    if (modules[locale]) {
      const mod = await modules[locale]();
      return mod.default ?? mod;
    }
  } catch {
    // Locale file missing — fall through to English fallback.
  }
  return enMessages;
}

/**
 * I18nProvider props.
 *
 * @typedef {Object} I18nProviderProps
 * @property {React.ReactNode} children
 * @property {string} [defaultLocale]   Override the auto-detected locale.
 */

/**
 * @param {I18nProviderProps} props
 */
export function I18nProvider({ children, defaultLocale }) {
  const [locale, setLocaleState] = useState(() => {
    return defaultLocale ?? detectLocale(['en']);
  });

  const [messages, setMessages] = useState(
    // Eagerly populate English so the UI never renders bare keys on first paint.
    locale === 'en' ? enMessages : {}
  );

  // Load locale dictionary whenever the locale changes.
  useEffect(() => {
    let cancelled = false;
    loadLocale(locale).then((dict) => {
      if (!cancelled) setMessages(dict);
    });
    return () => { cancelled = true; };
  }, [locale]);

  const setLocale = useCallback((newLocale) => {
    setLocaleState(newLocale);
  }, []);

  const value = useMemo(
    () => ({
      messages,
      fallback: enMessages,
      locale,
      setLocale,
    }),
    [messages, locale, setLocale]
  );

  return (
    <I18nContext.Provider value={value}>
      {children}
    </I18nContext.Provider>
  );
}

export default I18nProvider;
