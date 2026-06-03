import './Button.css';

const variantClasses = {
  primary: 'variantPrimary',
  secondary: 'variantSecondary',
  ghost: 'variantGhost',
  danger: 'variantDanger',
};

const sizeClasses = {
  sm: 'sizeSm',
  md: 'sizeMd',
  lg: 'sizeLg',
};

const spinnerSizeClasses = {
  sm: 'spinnerSm',
  md: 'spinnerMd',
  lg: 'spinnerLg',
};

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
}) {
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
