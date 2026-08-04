import type { ReactNode } from 'react';

export type BadgeVariant = 'solid' | 'outline' | 'subtle';
export type BadgeColor = 'amber' | 'green' | 'red' | 'blue' | 'gray';
export type BadgeSize = 'sm' | 'md' | 'lg';

const variantClasses: Record<BadgeVariant, Record<BadgeColor, string>> = {
  solid: {
    amber: 'badge-solid-amber',
    green: 'badge-solid-green',
    red: 'badge-solid-red',
    blue: 'badge-solid-blue',
    gray: 'badge-solid-gray',
  },
  outline: {
    amber: 'badge-outline-amber',
    green: 'badge-outline-green',
    red: 'badge-outline-red',
    blue: 'badge-outline-blue',
    gray: 'badge-outline-gray',
  },
  subtle: {
    amber: 'badge-subtle-amber',
    green: 'badge-subtle-green',
    red: 'badge-subtle-red',
    blue: 'badge-subtle-blue',
    gray: 'badge-subtle-gray',
  },
};

const sizeClasses: Record<BadgeSize, string> = {
  sm: 'badge-sm',
  md: 'badge-md',
  lg: 'badge-lg',
};

export interface BadgeProps {
  children?: ReactNode;
  variant?: BadgeVariant;
  color?: BadgeColor;
  size?: BadgeSize;
  dot?: boolean;
  className?: string;
}

export default function Badge({
  children,
  variant = 'solid',
  color = 'amber',
  size = 'md',
  dot = false,
  className = '',
}: BadgeProps) {
  const colorVariants = variantClasses[variant] || variantClasses.solid;
  const colorClass = colorVariants[color] || colorVariants.amber;
  const sizeClass = sizeClasses[size] || sizeClasses.md;

  const classes = [
    'badge',
    colorClass,
    sizeClass,
    className,
  ].filter(Boolean).join(' ');

  if (dot) {
    return (
      <span className={classes}>
        <span className="badge-dot" />
      </span>
    );
  }

  return (
    <span className={classes}>
      {children}
    </span>
  );
}
