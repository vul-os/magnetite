import './Card.css';

const variantClasses = {
  default: 'card-default',
  elevated: 'card-elevated',
  interactive: 'card-interactive',
  glass: 'card-glass',
};

const paddingClasses = {
  none: 'padding-none',
  sm: 'padding-sm',
  md: 'padding-md',
  lg: 'padding-lg',
};

export default function Card({
  children,
  variant = 'default',
  padding = 'md',
  onClick,
  hoverable = false,
  className = '',
  ...props
}) {
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

export function CardHeader({ children, className = '' }) {
  return (
    <div className={`card-header ${className}`}>
      {children}
    </div>
  );
}

export function CardBody({ children, className = '' }) {
  return (
    <div className={`card-body ${className}`}>
      {children}
    </div>
  );
}

export function CardFooter({ children, className = '' }) {
  return (
    <div className={`card-footer ${className}`}>
      {children}
    </div>
  );
}
