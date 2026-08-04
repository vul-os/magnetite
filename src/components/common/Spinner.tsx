export default function Spinner({ size = 'md', className = '' }) {
  const sizeClass = `spinner-${size}`;
  return (
    <span className={`spinner ${sizeClass} ${className}`.trim()} />
  );
}