import './Skeleton.css';

export default function Skeleton({
  variant = 'text',
  width,
  height,
  className = ''
}) {
  const classes = [
    'skeleton',
    `variant-${variant}`,
    className
  ].filter(Boolean).join(' ');

  const style = {};
  if (width) style.width = width;
  if (height) style.height = height;

  return <div className={classes} style={style} />;
}

export function SkeletonText({ lines = 3, className = '' }) {
  return (
    <div className={`skeleton-text ${className}`}>
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton
          key={i}
          variant="text"
          width={i === lines - 1 ? '70%' : '100%'}
        />
      ))}
    </div>
  );
}

export function SkeletonCard({ className = '' }) {
  return (
    <div className={`skeleton-card ${className}`}>
      <div className="skeleton-card-image" />
      <div className="skeleton-card-content">
        <Skeleton variant="text" width="80%" height={20} />
        <Skeleton variant="text" width="50%" height={14} />
        <div className="skeleton-card-footer">
          <Skeleton variant="text" width={60} height={20} />
          <Skeleton variant="text" width={80} height={16} />
        </div>
      </div>
    </div>
  );
}

export function SkeletonAvatar({ size = 'md', className = '' }) {
  const sizeMap = { sm: 32, md: 48, lg: 64 };
  const dimension = sizeMap[size] || sizeMap.md;
  return (
    <Skeleton
      variant="avatar"
      width={dimension}
      height={dimension}
      className={className}
    />
  );
}

export function SkeletonTableRow({ columns = 4, className = '' }) {
  return (
    <div className={`skeleton-table-row ${className}`}>
      {Array.from({ length: columns }).map((_, i) => (
        <Skeleton key={i} variant="text" />
      ))}
    </div>
  );
}
