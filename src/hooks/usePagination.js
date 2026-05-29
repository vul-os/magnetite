import { useState, useCallback, useMemo } from 'react';

export function usePagination({ total, perPage = 10, currentPage: initialPage = 1 }) {
  const [currentPage, setCurrentPage] = useState(initialPage);

  const totalPages = useMemo(() => {
    if (total <= 0 || perPage <= 0) return 0;
    return Math.ceil(total / perPage);
  }, [total, perPage]);

  const startIndex = useMemo(() => {
    return Math.max(0, (currentPage - 1) * perPage);
  }, [currentPage, perPage]);

  const endIndex = useMemo(() => {
    return Math.min(currentPage * perPage, total);
  }, [currentPage, perPage, total]);

  const hasNext = useMemo(() => {
    return currentPage < totalPages;
  }, [currentPage, totalPages]);

  const hasPrev = useMemo(() => {
    return currentPage > 1;
  }, [currentPage]);

  const goTo = useCallback((page) => {
    const pageNum = Math.max(1, Math.min(page, totalPages));
    setCurrentPage(pageNum);
  }, [totalPages]);

  const next = useCallback(() => {
    if (hasNext) {
      setCurrentPage(p => p + 1);
    }
  }, [hasNext]);

  const prev = useCallback(() => {
    if (hasPrev) {
      setCurrentPage(p => p - 1);
    }
  }, [hasPrev]);

  const reset = useCallback(() => {
    setCurrentPage(1);
  }, []);

  return {
    totalPages,
    currentPage,
    startIndex,
    endIndex,
    hasNext,
    hasPrev,
    goTo,
    next,
    prev,
    reset,
    setPage: goTo,
  };
}
