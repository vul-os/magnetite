import { memo } from 'react';
import { initialsAvatar } from '../utils/initialsAvatar';

const StarRatingDisplay = memo(function StarRatingDisplay({ rating, size = 'md' }) {
  const starSize = size === 'sm' ? 14 : size === 'lg' ? 24 : 18;

  return (
    <div className="star-rating-display">
      {[1, 2, 3, 4, 5].map((star) => (
        <svg
          key={star}
          className={`star ${star <= rating ? 'filled' : 'empty'}`}
          viewBox="0 0 24 24"
          fill="currentColor"
          style={{ width: starSize, height: starSize }}
        >
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
    </div>
  );
});

function ReviewCard({ review }) {
  return (
    <div className="review-card">
      <div className="review-header">
        <img
          src={review.user?.avatar || initialsAvatar(review.user?.username)}
          alt={review.user?.username || 'User'}
          className="review-avatar"
        />
        <div className="review-info">
          <span className="review-username">{review.user?.username || 'Anonymous'}</span>
          <StarRatingDisplay rating={review.rating} size="sm" />
        </div>
      </div>
      <p className="review-text">{review.content}</p>
    </div>
  );
}

export default memo(ReviewCard);
