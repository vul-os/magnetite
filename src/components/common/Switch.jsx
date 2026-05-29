import './Switch.css';

const sizeClasses = {
  sm: 'sizeSm',
  md: 'sizeMd',
};

export default function Switch({
  checked = false,
  onChange,
  disabled = false,
  size = 'md',
  className = '',
  ...props
}) {
  const classes = [
    'switch',
    sizeClasses[size],
    checked ? 'checked' : '',
    disabled ? 'disabled' : '',
    className,
  ].filter(Boolean).join(' ');

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={classes}
      disabled={disabled}
      onClick={() => onChange?.(!checked)}
      {...props}
    >
      <span className="switch-track">
        <span className="switch-thumb" />
      </span>
    </button>
  );
}
