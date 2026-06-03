/**
 * useTranslation — lightweight i18n hook.
 *
 * Returns a `t(key)` function that resolves dot-separated keys from the
 * active locale dictionary.  Falls back to the English string when the key
 * is missing in the active locale.  Falls back to the key itself when the
 * key is missing in both locales.
 *
 * Usage:
 *   import { useTranslation } from '../i18n/useTranslation';
 *   const { t } = useTranslation();
 *   <h1>{t('nav.home')}</h1>
 *
 * This is a SCAFFOLD — wiring to existing pages is deferred; the hook is
 * available for new code and future page migrations.
 */

import { useContext } from 'react';
import { I18nContext } from './I18nProvider';
import en from './en.json';

/**
 * Resolve a dot-separated key like "auth.errors.invalidCredentials"
 * against a nested object.  Returns undefined if the path does not exist.
 *
 * @param {object} obj  - The translation dictionary.
 * @param {string} key  - Dot-separated key path.
 * @returns {string|undefined}
 */
function resolvePath(obj, key) {
  return key.split('.').reduce((current, segment) => {
    return current != null && typeof current === 'object' ? current[segment] : undefined;
  }, obj);
}

/**
 * @typedef {Object} UseTranslationResult
 * @property {(key: string, vars?: Record<string, string|number>) => string} t
 *   Translate `key` with optional string-interpolation variables.
 * @property {string} locale  Active locale identifier (e.g. "en").
 * @property {(locale: string) => void} setLocale  Switch the active locale.
 */

/**
 * useTranslation hook.
 *
 * @returns {UseTranslationResult}
 */
export function useTranslation() {
  const ctx = useContext(I18nContext);

  if (!ctx) {
    // Fallback when used outside an I18nProvider (e.g. in tests without wrapper):
    // resolve against the bundled English dictionary so the UI still shows real
    // copy (not raw keys), then fall back to the key only if truly missing.
    return {
      t: (key, vars) => {
        const value = resolvePath(en, key);
        if (typeof value === 'string') {
          return vars
            ? value.replace(/\{\{(\w+)\}\}/g, (_, name) =>
                name in vars ? String(vars[name]) : `{{${name}}}`,
              )
            : value;
        }
        return key;
      },
      locale: 'en',
      setLocale: () => {},
    };
  }

  const { messages, fallback, locale, setLocale } = ctx;

  /**
   * Translate a key, with optional variable interpolation.
   *
   * Variables are substituted using `{{varName}}` syntax:
   *   t('common.greeting', { name: 'Alice' })
   *   // "Hello, Alice!"  (if en.json has "common.greeting": "Hello, {{name}}!")
   *
   * @param {string} key
   * @param {Record<string, string|number>} [vars]
   * @returns {string}
   */
  function t(key, vars) {
    // Try active locale first, then English fallback, then the key itself.
    let value = resolvePath(messages, key) ?? resolvePath(fallback, key) ?? key;

    // Simple {{variable}} interpolation.
    if (vars && typeof value === 'string') {
      value = value.replace(/\{\{(\w+)\}\}/g, (_, name) => {
        return name in vars ? String(vars[name]) : `{{${name}}}`;
      });
    }

    return typeof value === 'string' ? value : key;
  }

  return { t, locale, setLocale };
}
