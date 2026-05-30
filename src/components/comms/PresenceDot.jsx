/**
 * PresenceDot — small status indicator dot.
 * status: 'online' | 'idle' | 'dnd' | 'offline'
 */
export default function PresenceDot({ status = 'offline', size = 'sm', className = '' }) {
  const label =
    status === 'online'  ? 'Online' :
    status === 'idle'    ? 'Idle' :
    status === 'dnd'     ? 'Do not disturb' :
    'Offline';

  return (
    <span
      className={`presence-dot presence-dot--${status} presence-dot--${size} ${className}`}
      aria-label={label}
      role="img"
      title={label}
    />
  );
}
