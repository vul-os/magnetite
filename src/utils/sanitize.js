/**
 * sanitize.js — XSS-safe text helpers for user-generated content.
 *
 * React JSX text nodes are already XSS-safe (React escapes on render).
 * This module is the single, auditable choke-point for any situation
 * where content must be:
 *   - rendered via setAttribute / innerHTML (avoid if at all possible), or
 *   - truncated / normalised before display.
 *
 * Rules:
 *  - NEVER use dangerouslySetInnerHTML with raw user input.
 *  - Always prefer JSX text nodes ({variable}) over innerHTML.
 *  - Use escapeHtml() only when writing to the DOM imperatively.
 *  - Use sanitizeText() to normalise/trim user-supplied strings before display.
 */

/**
 * Escape a string so it is safe to inject into an HTML context via innerHTML.
 * Prefer JSX text nodes — this is a last resort for imperative DOM writes.
 *
 * @param {string} str - Raw user-supplied string.
 * @returns {string} HTML-escaped string.
 */
export function escapeHtml(str) {
  if (str == null) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#x27;')
    .replace(/\//g, '&#x2F;');
}

/**
 * Normalise a user-supplied text string for safe display as a JSX text node.
 * Trims whitespace and collapses internal runs of whitespace/newlines.
 * The result is safe to use as `{sanitizeText(value)}` in JSX.
 *
 * @param {string} str - Raw user-supplied string.
 * @param {number} [maxLength] - Optional maximum character length (truncates with ellipsis).
 * @returns {string} Normalised string.
 */
export function sanitizeText(str, maxLength) {
  if (str == null) return '';
  let out = String(str).trim();
  if (maxLength != null && out.length > maxLength) {
    out = out.slice(0, maxLength) + '…'; // …
  }
  return out;
}

/**
 * Validate that a redirect destination is a safe same-origin relative path.
 * Rejects absolute URLs and protocol-relative URLs (//evil.com).
 *
 * @param {string|null|undefined} destination - The destination string from a query param or form field.
 * @param {string} [fallback='/'] - Returned when destination is unsafe.
 * @returns {string} A safe relative path.
 */
export function sanitizeRedirect(destination, fallback = '/') {
  if (!destination) return fallback;
  const d = String(destination);
  // Must start with exactly one '/' and not be a protocol-relative URL ('//...').
  if (d.startsWith('/') && !d.startsWith('//') && !d.startsWith('/\\')) {
    // Additional guard: reject anything that looks like it contains a protocol.
    if (/^\/[^/]/.test(d) || d === '/') return d;
  }
  return fallback;
}
