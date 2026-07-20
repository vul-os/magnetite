/**
 * Unavailable / LoadError — the two honest failure surfaces.
 *
 * DESIGN.md §7.2 draws a hard line between three states:
 *   .state-empty        there is genuinely nothing yet
 *   .state-error        the request failed  → <LoadError>
 *   .state-unavailable  this node has no backend for this capability → <Unavailable>
 *
 * The distinction matters. "Failed to load" invites a retry; "not built" does
 * not, and a retry button on a capability that was never implemented is a lie
 * told once per click.
 *
 * The signature of <Unavailable> is the **capability manifest**: it names the
 * exact routes that are not mounted, in mono, the way the rest of this UI
 * renders anything a user might copy or verify. On a self-hostable node that is
 * not decoration — it is the fact an operator needs.
 */
import './Unavailable.css';

/* ── Icons ────────────────────────────────────────────────────────────────── */

/* Unavailable: an open circuit — a line that stops. Not a warning. */
function AbsentIcon() {
  return (
    <svg className="state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
      <path d="M2 12h6" />
      <path d="M16 12h6" />
      <circle cx="9.5" cy="12" r="1.5" />
      <circle cx="14.5" cy="12" r="1.5" />
      <path d="M11.5 8.5v7" strokeDasharray="1 3" />
    </svg>
  );
}

/* Error: a genuine alert. */
function AlertIcon() {
  return (
    <svg className="state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
      <path d="M12 9v4" />
      <path d="M12 17h.01" />
    </svg>
  );
}

/* ── Heading ──────────────────────────────────────────────────────────────── */

/**
 * Headings must not skip levels (DESIGN.md §8). Callers pass the level that is
 * correct for where the state sits in the document, so a page-level state is an
 * <h2> under the page <h1> and a panel-level state is an <h3>.
 */
function StateHeading({ level = 2, className, children }) {
  const Tag = `h${Math.min(Math.max(level, 1), 6)}`;
  return <Tag className={className}>{children}</Tag>;
}

/* ── Unavailable ──────────────────────────────────────────────────────────── */

/**
 * A capability that is not implemented on this node.
 *
 * @param {string}   title      What is missing, in plain words.
 * @param {node}     children   Why, and what the user can do instead.
 * @param {string[]} endpoints  The literal unmounted routes, e.g.
 *                              ['GET /api/v1/points/rewards'].
 * @param {number}   headingLevel  Document-correct heading level (default 2).
 * @param {node}     actions    Real destinations only — never a retry.
 * @param {boolean}  inline     Compact variant, for one unavailable action
 *                              inside an otherwise working page.
 */
export function Unavailable({
  title = 'Not built yet',
  children,
  endpoints,
  headingLevel = 2,
  actions,
  inline = false,
  className = '',
  ...rest
}) {
  const cls = [
    inline ? 'state-inline' : 'state',
    'state-unavailable',
    'edge-spec',
    className,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={cls} {...rest}>
      {!inline && <AbsentIcon />}
      <p className="m-xs state-kicker">Capability absent</p>
      <StateHeading level={headingLevel} className="state-title">
        {title}
      </StateHeading>
      {children && <div className="state-body">{children}</div>}
      {endpoints?.length > 0 && (
        <ul className="state-manifest" aria-label="Endpoints not mounted on this node">
          {endpoints.map((route) => (
            <li key={route}>
              <code className="state-route">{route}</code>
              <span className="state-route-verdict">not mounted</span>
            </li>
          ))}
        </ul>
      )}
      {actions && <div className="state-actions">{actions}</div>}
    </div>
  );
}

/* ── LoadError ────────────────────────────────────────────────────────────── */

/**
 * A request that failed. Distinct from <Unavailable>: the capability exists,
 * this attempt did not land, and retrying is a real option.
 */
export function LoadError({
  title = 'Could not load',
  children,
  detail,
  onRetry,
  headingLevel = 2,
  className = '',
  ...rest
}) {
  return (
    <div className={['state', 'state-error', className].filter(Boolean).join(' ')} {...rest}>
      <AlertIcon />
      <p className="m-xs state-kicker">Request failed</p>
      <StateHeading level={headingLevel} className="state-title">
        {title}
      </StateHeading>
      {children && <div className="state-body">{children}</div>}
      {detail && <p className="state-detail mono">{detail}</p>}
      {onRetry && (
        <div className="state-actions">
          <button type="button" className="btn btn-secondary btn-sm" onClick={onRetry}>
            Try again
          </button>
        </div>
      )}
    </div>
  );
}

export default Unavailable;
