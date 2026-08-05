import { forwardRef, type ReactNode, type UIEvent } from 'react';
import Spinner from './common/Spinner';

interface InfiniteScrollProps {
  children?: ReactNode;
  hasMore?: boolean;
  isLoading?: boolean;
  onLoadMore?: () => void;
  loadingComponent?: ReactNode;
  threshold?: number;
  className?: string;
}

const InfiniteScroll = forwardRef<HTMLDivElement, InfiniteScrollProps>(function InfiniteScroll({
  children,
  hasMore = true,
  isLoading = false,
  onLoadMore,
  loadingComponent,
  threshold = 100,
  className = '',
}, ref) {
  const handleScroll = (e: UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = e.target as HTMLDivElement;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;

    if (distanceFromBottom < threshold && hasMore && !isLoading && onLoadMore) {
      onLoadMore();
    }
  };

  return (
    <div
      ref={ref}
      className={`infinite-scroll-container ${className}`}
      onScroll={handleScroll}
    >
      {children}

      {isLoading && (
        <div className="infinite-scroll-loader">
          {loadingComponent || <Spinner size="md" />}
          <span>Loading more...</span>
        </div>
      )}

      {!hasMore && !isLoading && (
        <div className="infinite-scroll-end">
          <span>No more items to load</span>
        </div>
      )}
    </div>
  );
});

export default InfiniteScroll;
