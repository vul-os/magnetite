import type { CSSProperties } from 'react';
import './Skeleton.css';

interface SkeletonProps {
  variant?: string;
  width?: string | number;
  height?: string | number;
  borderRadius?: string | number;
  className?: string;
}

export default function Skeleton({
  variant = 'text',
  width,
  height,
  borderRadius,
  className = ''
}: SkeletonProps) {
  const classes = [
    'skeleton',
    `variant-${variant}`,
    className
  ].filter(Boolean).join(' ');

  const style: CSSProperties = {};
  if (width) style.width = width;
  if (height) style.height = height;
  if (borderRadius) style.borderRadius = borderRadius;

  return <div className={classes} style={style} />;
}

interface SkeletonTextProps {
  lines?: number;
  className?: string;
}

export function SkeletonText({ lines = 3, className = '' }: SkeletonTextProps) {
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
