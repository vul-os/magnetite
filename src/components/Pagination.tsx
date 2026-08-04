import { useMemo } from 'react';
import Button from './common/Button';

export default function Pagination({
  total = 0,
  perPage = 10,
  currentPage = 1,
  onPageChange,
  showFirstLast = true,
  showPerPageSelector = false,
  perPageOptions = [10, 25, 50, 100],
  onPerPageChange,
  compact = false,
  className = '',
}) {
  const totalPages = useMemo(() => {
    if (total <= 0 || perPage <= 0) return 0;
    return Math.ceil(total / perPage);
  }, [total, perPage]);

  const startIndex = useMemo(() => {
    if (totalPages === 0) return 0;
    return Math.max(1, (currentPage - 1) * perPage + 1);
  }, [currentPage, perPage, totalPages]);

  const endIndex = useMemo(() => {
    return Math.min(currentPage * perPage, total);
  }, [currentPage, perPage, total]);

  const pageNumbers = useMemo(() => {
    if (compact || totalPages <= 7) {
      return Array.from({ length: totalPages }, (_, i) => i + 1);
    }

    const pages = [];
    const showLeftEllipsis = currentPage > 3;
    const showRightEllipsis = currentPage < totalPages - 2;

    if (showLeftEllipsis) {
      pages.push(1, '...');
    } else {
      for (let i = 1; i <= Math.min(3, totalPages); i++) {
        pages.push(i);
      }
    }

    const start = showLeftEllipsis ? Math.max(4, currentPage - 1) : 4;
    const end = showRightEllipsis ? Math.min(totalPages - 1, currentPage + 1) : Math.min(totalPages, 3);

    for (let i = start; i <= end; i++) {
      if (!pages.includes(i)) {
        pages.push(i);
      }
    }

    if (showRightEllipsis) {
      if (!pages.includes(totalPages)) {
        pages.push('...', totalPages);
      }
    }

    return pages;
  }, [currentPage, totalPages, compact]);

  const handlePageClick = (page) => {
    if (page < 1 || page > totalPages || page === currentPage) return;
    onPageChange?.(page);
  };

  const handlePerPageChange = (e) => {
    const newPerPage = parseInt(e.target.value, 10);
    onPerPageChange?.(newPerPage);
  };

  if (totalPages <= 1 && !showPerPageSelector) {
    return null;
  }

  return (
    <div className={`pagination-container ${compact ? 'compact' : ''} ${className}`}>
      {!compact && (
        <div className="pagination-info">
          <span className="showing-text">
            Showing {startIndex}-{endIndex} of {total}
          </span>
        </div>
      )}

      <div className="pagination-controls">
        {showFirstLast && !compact && totalPages > 1 && (
          <Button
            variant="ghost"
            size="sm"
            disabled={currentPage === 1}
            onClick={() => handlePageClick(1)}
            className="page-btn first-last"
          >
            First
          </Button>
        )}

        <Button
          variant="ghost"
          size="sm"
          disabled={currentPage === 1}
          onClick={() => handlePageClick(currentPage - 1)}
          className="page-btn prev-next"
        >
          Previous
        </Button>

        <div className="page-numbers">
          {pageNumbers.map((page, index) => (
            page === '...' ? (
              <span key={`ellipsis-${index}`} className="ellipsis">...</span>
            ) : (
              <button
                key={page}
                className={`page-number ${currentPage === page ? 'active' : ''}`}
                onClick={() => handlePageClick(page)}
              >
                {page}
              </button>
            )
          ))}
        </div>

        <Button
          variant="ghost"
          size="sm"
          disabled={currentPage === totalPages}
          onClick={() => handlePageClick(currentPage + 1)}
          className="page-btn prev-next"
        >
          Next
        </Button>

        {showFirstLast && !compact && totalPages > 1 && (
          <Button
            variant="ghost"
            size="sm"
            disabled={currentPage === totalPages}
            onClick={() => handlePageClick(totalPages)}
            className="page-btn first-last"
          >
            Last
          </Button>
        )}
      </div>

      {showPerPageSelector && (
        <div className="per-page-selector">
          <label htmlFor="per-page-select">Items per page:</label>
          <select
            id="per-page-select"
            value={perPage}
            onChange={handlePerPageChange}
          >
            {perPageOptions.map(option => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}
