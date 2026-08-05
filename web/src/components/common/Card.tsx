import type { MouseEventHandler, ReactNode } from 'react';
import './Card.css';

export type CardVariant = 'default' | 'elevated' | 'interactive' | 'glass';
export type CardPadding = 'none' | 'sm' | 'md' | 'lg';

const variantClasses: Record<CardVariant, string> = {
  default: 'card-default',
  elevated: 'card-elevated',
  interactive: 'card-interactive',
  glass: 'card-glass',
};

const paddingClasses: Record<CardPadding, string> = {
  none: 'padding-none',
  sm: 'padding-sm',
  md: 'padding-md',
  lg: 'padding-lg',
};

export interface CardProps {
  children?: ReactNode;
  variant?: CardVariant;
  padding?: CardPadding;
  onClick?: MouseEventHandler<HTMLButtonElement | HTMLDivElement>;
  hoverable?: boolean;
  className?: string;
  [key: string]: unknown;
}

export default function Card({
  children,
  variant = 'default',
  padding = 'md',
  onClick,
  hoverable = false,
  className = '',
  ...props
}: CardProps) {
  const isInteractive = hoverable || onClick;

  const classes = [
    'card',
    variantClasses[variant],
    paddingClasses[padding],
    isInteractive ? 'card-hoverable' : '',
    onClick ? 'card-clickable' : '',
    className,
  ].filter(Boolean).join(' ');

  const Component = onClick ? 'button' : 'div';

  return (
    <Component
      className={classes}
      onClick={onClick}
      {...props}
    >
      {children}
    </Component>
  );
}

export interface CardSectionProps {
  children?: ReactNode;
  className?: string;
}

export function CardHeader({ children, className = '' }: CardSectionProps) {
  return (
    <div className={`card-header ${className}`}>
      {children}
    </div>
  );
}

export function CardBody({ children, className = '' }: CardSectionProps) {
  return (
    <div className={`card-body ${className}`}>
      {children}
    </div>
  );
}

export function CardFooter({ children, className = '' }: CardSectionProps) {
  return (
    <div className={`card-footer ${className}`}>
      {children}
    </div>
  );
}
