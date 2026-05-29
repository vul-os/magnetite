import { forwardRef } from 'react';
import Spinner from './common/Spinner';

const InfiniteScroll = forwardRef(function InfiniteScroll({
  children,
  hasMore = true,
  isLoading = false,
  onLoadMore,
  loadingComponent,
  threshold = 100,
  className = '',
}, ref) {
  const handleScroll = (e) => {
    const { scrollTop, scrollHeight, clientHeight } = e.target;
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
