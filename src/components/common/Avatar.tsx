import { memo, type HTMLAttributes } from 'react';

export type AvatarSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';
// `string` alone would swallow the 4 named literals (TS widens the whole
// union to `string`, losing autocomplete for the known statuses); the
// `string & {}` idiom keeps the literals suggestable while still accepting
// any other string value.
export type AvatarStatus = 'online' | 'idle' | 'dnd' | 'offline' | (string & {});

const sizeMap: Record<AvatarSize, number> = {
  xs: 24,
  sm: 32,
  md: 40,
  lg: 56,
  xl: 80,
};

function getInitials(name?: string | null): string {
  if (!name) return '?';
  const parts = name.trim().split(' ');
  if (parts.length === 1) return parts[0].charAt(0).toUpperCase();
  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

export interface AvatarProps extends HTMLAttributes<HTMLDivElement> {
  src?: string;
  alt?: string;
  name?: string;
  size?: AvatarSize;
  showOnline?: boolean;
  status?: AvatarStatus;
  className?: string;
}

export default memo(function Avatar({
  src,
  alt = '',
  name,
  size = 'md',
  showOnline = false,
  status,
  className = '',
  ...props
}: AvatarProps) {
  const dimension = sizeMap[size];
  const initials = getInitials(name || alt);

  const hasImage = Boolean(src);

  const classes = [
    'avatar',
    `avatar-${size}`,
    hasImage ? 'avatar-image' : 'avatar-initials',
    status ? `avatar-status-${status}` : '',
    className,
  ].filter(Boolean).join(' ');

  return (
    <div className={classes} style={{ width: dimension, height: dimension }} {...props}>
      {hasImage ? (
        <img src={src} alt={alt || name || 'Avatar'} className="avatar-img" loading="lazy" />
      ) : (
        <span className="avatar-initials-text" aria-hidden="true">{initials}</span>
      )}
      {showOnline && (
        <span
          className="avatar-online-indicator"
          aria-label="Online"
          role="img"
        />
      )}
      {status && <span className="avatar-status-ring" aria-hidden="true" />}
    </div>
  );
});
