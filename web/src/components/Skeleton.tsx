import type { CSSProperties } from 'react';
import './Skeleton.css';

export interface SkeletonProps {
  variant?: string;
  width?: number | string;
  height?: number | string;
  className?: string;
}

export default function Skeleton({
  variant = 'text',
  width,
  height,
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

  return <div className={classes} style={style} />;
}

export interface SkeletonTextProps {
  lines?: number;
  className?: string;
}

export function SkeletonText({ lines = 3, className = '' }: SkeletonTextProps) {
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

export interface SkeletonCardProps {
  className?: string;
}

export function SkeletonCard({ className = '' }: SkeletonCardProps) {
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

export interface SkeletonAvatarProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function SkeletonAvatar({ size = 'md', className = '' }: SkeletonAvatarProps) {
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

export interface SkeletonTableRowProps {
  columns?: number;
  className?: string;
}

export function SkeletonTableRow({ columns = 4, className = '' }: SkeletonTableRowProps) {
  return (
    <div className={`skeleton-table-row ${className}`}>
      {Array.from({ length: columns }).map((_, i) => (
        <Skeleton key={i} variant="text" />
      ))}
    </div>
  );
}
