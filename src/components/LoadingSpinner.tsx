import './LoadingSpinner.css';

export default function LoadingSpinner({
  size = 'md',
  color = 'primary',
  centered = false,
  inline = false,
  className = ''
}) {
  const classes = [
    'loading-spinner',
    `size-${size}`,
    `color-${color}`,
    centered && 'centered',
    inline && 'inline',
    className
  ].filter(Boolean).join(' ');

  return (
    <div className={classes}>
      <div className="spinner-track">
        <div className="spinner-rotator" />
      </div>
    </div>
  );
}
