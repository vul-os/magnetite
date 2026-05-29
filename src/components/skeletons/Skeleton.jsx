import './Skeleton.css';

export default function Skeleton({
  variant = 'text',
  width,
  height,
  borderRadius,
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
  if (borderRadius) style.borderRadius = borderRadius;

  return <div className={classes} style={style} />;
}

export function SkeletonText({ lines = 3, className = '' }) {
  return (
    <div className={`skeleton-text-lines ${className}`}>
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
