import { useState, useEffect, useCallback, useRef } from 'react';

export interface UseInfiniteScrollOptions {
  fetchMore: () => Promise<void>;
  hasMore: boolean;
  threshold?: number;
}

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

export function useInfiniteScroll({ fetchMore, hasMore, threshold = 100 }: UseInfiniteScrollOptions) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const observerRef = useRef<IntersectionObserver | null>(null);
  const sentinelRef = useRef<Element | null>(null);

  const loadMore = useCallback(async () => {
    if (loading || !hasMore) return;

    setLoading(true);
    setError(null);

    try {
      await fetchMore();
    } catch (err) {
      setError(errMessage(err, 'Failed to load more'));
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
          void loadMore();
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

  const setSentinelRef = useCallback((node: Element | null) => {
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
