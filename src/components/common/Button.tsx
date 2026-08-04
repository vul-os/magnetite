import type { ButtonHTMLAttributes, ReactNode } from 'react';
import './Button.css';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

const variantClasses: Record<ButtonVariant, string> = {
  primary: 'variantPrimary',
  secondary: 'variantSecondary',
  ghost: 'variantGhost',
  danger: 'variantDanger',
};

const sizeClasses: Record<ButtonSize, string> = {
  sm: 'sizeSm',
  md: 'sizeMd',
  lg: 'sizeLg',
};

const spinnerSizeClasses: Record<ButtonSize, string> = {
  sm: 'spinnerSm',
  md: 'spinnerMd',
  lg: 'spinnerLg',
};

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'> {
  children?: ReactNode;
  variant?: ButtonVariant;
  size?: ButtonSize;
  isLoading?: boolean;
  isDisabled?: boolean;
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  type?: 'button' | 'submit' | 'reset';
}

export default function Button({
  children,
  variant = 'primary',
  size = 'md',
  isLoading = false,
  isDisabled = false,
  leftIcon,
  rightIcon,
  type = 'button',
  onClick,
  className = '',
  'aria-label': ariaLabel,
  ...props
}: ButtonProps) {
  const classes = [
    'button',
    variantClasses[variant],
    sizeClasses[size],
    isLoading ? 'loading' : '',
    className,
  ].filter(Boolean).join(' ');

  return (
    <button
      type={type}
      className={classes}
      disabled={isDisabled || isLoading}
      onClick={onClick}
      aria-label={ariaLabel}
      aria-busy={isLoading || undefined}
      aria-disabled={isDisabled || isLoading || undefined}
      {...props}
    >
      {isLoading && (
        <span
          className={`spinner ${spinnerSizeClasses[size]}`}
          aria-hidden="true"
        />
      )}
      {leftIcon && !isLoading && (
        <span className="icon iconLeft" aria-hidden="true">{leftIcon}</span>
      )}
      {children}
      {rightIcon && !isLoading && (
        <span className="icon iconRight" aria-hidden="true">{rightIcon}</span>
      )}
    </button>
  );
}
