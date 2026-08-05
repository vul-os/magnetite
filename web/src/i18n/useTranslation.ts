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
import { I18nContext, type Messages } from './I18nProvider';
import en from './en.json';

export type TranslationVars = Record<string, string | number>;

/**
 * Resolve a dot-separated key like "auth.errors.invalidCredentials"
 * against a nested object.  Returns undefined if the path does not exist.
 */
function resolvePath(obj: Messages, key: string): unknown {
  return key.split('.').reduce<unknown>((current, segment) => {
    return current != null && typeof current === 'object'
      ? (current as Record<string, unknown>)[segment]
      : undefined;
  }, obj);
}

export interface UseTranslationResult {
  /** Translate `key` with optional string-interpolation variables. */
  t: (key: string, vars?: TranslationVars) => string;
  /** Active locale identifier (e.g. "en"). */
  locale: string;
  /** Switch the active locale. */
  setLocale: (locale: string) => void;
}

/**
 * useTranslation hook.
 */
export function useTranslation(): UseTranslationResult {
  const ctx = useContext(I18nContext);

  if (!ctx) {
    // Fallback when used outside an I18nProvider (e.g. in tests without wrapper):
    // resolve against the bundled English dictionary so the UI still shows real
    // copy (not raw keys), then fall back to the key only if truly missing.
    return {
      t: (key: string, vars?: TranslationVars) => {
        const value = resolvePath(en, key);
        if (typeof value === 'string') {
          return vars
            ? value.replace(/\{\{(\w+)\}\}/g, (_, name: string) =>
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
   */
  function t(key: string, vars?: TranslationVars): string {
    // Try active locale first, then English fallback, then the key itself.
    let value: unknown = resolvePath(messages, key) ?? resolvePath(fallback, key) ?? key;

    // Simple {{variable}} interpolation.
    if (vars && typeof value === 'string') {
      value = value.replace(/\{\{(\w+)\}\}/g, (_, name: string) => {
        return name in vars ? String(vars[name]) : `{{${name}}}`;
      });
    }

    return typeof value === 'string' ? value : key;
  }

  return { t, locale, setLocale };
}
