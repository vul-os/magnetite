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
      {...props}
    >
      {isLoading && (
        <span className={`spinner ${spinnerSizeClasses[size]}`} />
      )}
      {leftIcon && !isLoading && (
        <span className="icon iconLeft">{leftIcon}</span>
      )}
      {children}
      {rightIcon && !isLoading && (
        <span className="icon iconRight">{rightIcon}</span>
      )}
    </button>
  );
}
