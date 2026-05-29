import { useState, useEffect, useCallback, useRef } from 'react';

export function useInfiniteScroll({ fetchMore, hasMore, threshold = 100 }) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const observerRef = useRef(null);
  const sentinelRef = useRef(null);

  const loadMore = useCallback(async () => {
    if (loading || !hasMore) return;

    setLoading(true);
    setError(null);

    try {
      await fetchMore();
    } catch (err) {
      setError(err.message || 'Failed to load more');
    } finally {
      setLoading(false);
    }
  }, [fetchMore, hasMore, loading]);

  useEffect(() => {
    if (observerRef.current) {
      observerRef.current.disconnect();
    }

    observerRef.current = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && hasMore && !loading) {
          loadMore();
        }
      },
      { rootMargin: `${threshold}px` }
    );

    if (sentinelRef.current) {
      observerRef.current.observe(sentinelRef.current);
    }

    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect();
      }
    };
  }, [hasMore, loading, loadMore, threshold]);

  const setSentinelRef = useCallback((node) => {
    if (sentinelRef.current) {
      observerRef.current?.unobserve(sentinelRef.current);
    }
    sentinelRef.current = node;
    if (node) {
      observerRef.current?.observe(node);
    }
  }, []);

  return {
    loading,
    error,
    sentinelRef: setSentinelRef,
    loadMore,
  };
}
