import { useState, useMemo } from 'react';
import ReviewCard from './ReviewCard';
import Pagination from './Pagination';
import Button from './common/Button';
import './ReviewList.css';

const SORT_OPTIONS = [
  { value: 'recent',     label: 'Most Recent' },
  { value: 'helpful',    label: 'Most Helpful' },
  { value: 'high_rating', label: 'Highest Rating' },
  { value: 'low_rating', label: 'Lowest Rating' },
];

const REVIEWS_PER_PAGE = 5;

export default function ReviewList({
  reviews = [],
  onHelpful,
  onReport,
  showCreateReview = false,
  onCreateReview,
  walletConnected = false,
}) {
  const [sortBy, setSortBy]             = useState('recent');
  const [currentPage, setCurrentPage]   = useState(1);
  const [votedReviews, setVotedReviews] = useState(new Set());

  const sortedReviews = useMemo(() => {
    const sorted = [...reviews];
    switch (sortBy) {
      case 'recent':      return sorted.sort((a, b) => new Date(b.date) - new Date(a.date));
      case 'helpful':     return sorted.sort((a, b) => (b.helpful || 0) - (a.helpful || 0));
      case 'high_rating': return sorted.sort((a, b) => b.rating - a.rating);
      case 'low_rating':  return sorted.sort((a, b) => a.rating - b.rating);
      default:            return sorted;
    }
  }, [reviews, sortBy]);

  const totalPages       = Math.ceil(sortedReviews.length / REVIEWS_PER_PAGE);
  const paginatedReviews = sortedReviews.slice(
    (currentPage - 1) * REVIEWS_PER_PAGE,
    currentPage * REVIEWS_PER_PAGE
  );

  const handleSortChange = (e) => {
    setSortBy(e.target.value);
    setCurrentPage(1);
  };

  const handleHelpful = (reviewId) => {
    if (votedReviews.has(reviewId)) return;
    setVotedReviews(prev => new Set([...prev, reviewId]));
    onHelpful?.(reviewId);
  };

  const handlePageChange = (page) => {
    setCurrentPage(page);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  if (reviews.length === 0) {
    return (
      <div className="review-list-empty">
        <div className="empty-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
          </svg>
        </div>
        <h3>No Reviews Yet</h3>
        <p>Be the first to share your thoughts about this game!</p>
        {showCreateReview && (
          <Button onClick={onCreateReview} variant="primary">
            Write a Review
          </Button>
        )}
      </div>
    );
  }

  return (
    <div className="review-list-component">
      <div className="review-list-header">
        <div className="sort-container">
          <label htmlFor="sort-reviews">Sort by:</label>
          <select
            id="sort-reviews"
            value={sortBy}
            onChange={handleSortChange}
            className="sort-select"
          >
            {SORT_OPTIONS.map(opt => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>

        {showCreateReview && (
          <Button
            onClick={onCreateReview}
            variant="primary"
            size="sm"
            isDisabled={!walletConnected}
          >
            {walletConnected ? 'Write a Review' : 'Connect Wallet to Review'}
          </Button>
        )}
      </div>

      <div className="reviews-container">
        {paginatedReviews.map(review => (
          <ReviewCard
            key={review.id ?? review.user}
            review={review}
            onHelpful={handleHelpful}
            onReport={onReport}
            isHelpfulVoted={votedReviews.has(review.id ?? review.user)}
          />
        ))}
      </div>

      {totalPages > 1 && (
        <Pagination
          total={sortedReviews.length}
          perPage={REVIEWS_PER_PAGE}
          currentPage={currentPage}
          onPageChange={handlePageChange}
          compact
        />
      )}
    </div>
  );
}
