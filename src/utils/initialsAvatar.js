/**
 * initialsAvatar — a locally-generated, deterministic avatar placeholder.
 *
 * Honesty: when an entity has no real avatar of its own we must NOT invent one
 * or pull a random external photo (e.g. picsum.photos) that misrepresents the
 * person. This returns an inline SVG data URI showing the entity's initial on a
 * neutral tile — clearly a placeholder, issuing zero network requests.
 *
 * Neutral tones are used deliberately: a placeholder graphic is not a themeable
 * stylesheet element (it cannot read CSS custom properties), so a mid-slate that
 * stays legible in both light and dark themes is the honest choice.
 */

const PLACEHOLDER_BG = '#31343c'; // neutral slate — legible in both themes
const PLACEHOLDER_FG = '#9aa1ad'; // muted ink

function firstInitial(name) {
  const trimmed = String(name ?? '').trim();
  if (!trimmed) return '?';
  return trimmed.charAt(0).toUpperCase();
}

/**
 * @param {string} name   entity name/username used for the initial
 * @param {number} size   pixel dimension of the square SVG (default 96)
 * @returns {string}      a `data:image/svg+xml,...` URI (no network request)
 */
export function initialsAvatar(name, size = 96) {
  const initial = firstInitial(name)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">` +
    `<rect width="${size}" height="${size}" fill="${PLACEHOLDER_BG}"/>` +
    `<text x="50%" y="50%" dy="0.35em" text-anchor="middle" ` +
    `font-family="IBM Plex Mono, ui-monospace, monospace" font-weight="600" ` +
    `font-size="${Math.round(size * 0.42)}" fill="${PLACEHOLDER_FG}">${initial}</text>` +
    `</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export default initialsAvatar;
