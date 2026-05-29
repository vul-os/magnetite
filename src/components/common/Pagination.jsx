import { useMemo } from 'react';
import './Pagination.css';

export default function Pagination({
  currentPage = 1,
  totalPages = 1,
  onPageChange,
  itemsPerPage = 10,
  onItemsPerPageChange,
  totalItems = 0,
}) {
  const startIndex = useMemo(() => {
    if (totalItems === 0) return 0;
    return (currentPage - 1) * itemsPerPage + 1;
  }, [currentPage, itemsPerPage, totalItems]);

  const endIndex = useMemo(() => {
    return Math.min(currentPage * itemsPerPage, totalItems);
  }, [currentPage, itemsPerPage, totalItems]);

  const pageNumbers = useMemo(() => {
    if (totalPages <= 7) {
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
  }, [currentPage, totalPages]);

  const handlePageClick = (page) => {
    if (page < 1 || page > totalPages || page === currentPage) return;
    onPageChange?.(page);
  };

  const handlePerPageChange = (e) => {
    const newPerPage = parseInt(e.target.value, 10);
    onItemsPerPageChange?.(newPerPage);
  };

  if (totalPages <= 1 && !onItemsPerPageChange) {
    return null;
  }

  return (
    <div className="pagination-container">
      <div className="pagination-info">
        <span className="showing-text">
          Showing {startIndex}-{endIndex} of {totalItems}
        </span>
      </div>

      <div className="pagination-controls">
        {totalPages > 1 && (
          <button
            className="pagination-btn first-last"
            disabled={currentPage === 1}
            onClick={() => handlePageClick(1)}
          >
            First
          </button>
        )}

        <button
          className="pagination-btn prev-next"
          disabled={currentPage === 1}
          onClick={() => handlePageClick(currentPage - 1)}
        >
          Previous
        </button>

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

        <button
          className="pagination-btn prev-next"
          disabled={currentPage === totalPages}
          onClick={() => handlePageClick(currentPage + 1)}
        >
          Next
        </button>

        {totalPages > 1 && (
          <button
            className="pagination-btn first-last"
            disabled={currentPage === totalPages}
            onClick={() => handlePageClick(totalPages)}
          >
            Last
          </button>
        )}
      </div>

      {onItemsPerPageChange && (
        <div className="per-page-selector">
          <label htmlFor="per-page-select">Items per page:</label>
          <select
            id="per-page-select"
            value={itemsPerPage}
            onChange={handlePerPageChange}
          >
            <option value={10}>10</option>
            <option value={25}>25</option>
            <option value={50}>50</option>
            <option value={100}>100</option>
          </select>
        </div>
      )}
    </div>
  );
}