/**
 * I18nProvider — lightweight React i18n context provider.
 *
 * Loads locale JSON files lazily and exposes them via I18nContext.
 * The English locale ("en") is the default and always loaded eagerly as
 * the fallback dictionary.
 *
 * Locale persistence: the active locale is stored in localStorage under the
 * key "magnetite_locale" and restored on the next page load.
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

/** localStorage key used to persist the user's locale choice. */
const LOCALE_STORAGE_KEY = 'magnetite_locale';

/** Supported locale codes (add new locales here when their JSON files are added). */
const SUPPORTED_LOCALES = ['en', 'es'];

/**
 * Read the persisted locale from localStorage, falling back to browser
 * detection and finally to "en".
 *
 * @returns {string}
 */
function readPersistedLocale() {
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (stored && SUPPORTED_LOCALES.includes(stored)) return stored;
  } catch {
    // localStorage may be unavailable (private browsing, etc.).
  }
  // Browser-language detection as secondary fallback.
  const preferred = (navigator?.language ?? 'en').split('-')[0].toLowerCase();
  return SUPPORTED_LOCALES.includes(preferred) ? preferred : 'en';
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
      'es': () => import('./es.json'),
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
    // Explicit prop takes precedence; otherwise restore from localStorage.
    return defaultLocale ?? readPersistedLocale();
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
    try {
      localStorage.setItem(LOCALE_STORAGE_KEY, newLocale);
    } catch {
      // Ignore write failures (private browsing, quota exceeded, etc.).
    }
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
