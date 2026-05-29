import { useState } from 'react';
import Button from './common/Button';

const MAX_CHARACTERS = 1000;

const StarRatingInput = ({ value, onChange }) => {
  const [hoverRating, setHoverRating] = useState(0);

  const handleClick = (rating) => {
    onChange(rating);
  };

  const handleMouseEnter = (rating) => {
    setHoverRating(rating);
  };

  const handleMouseLeave = () => {
    setHoverRating(0);
  };

  return (
    <div className="star-rating-input">
      {[1, 2, 3, 4, 5].map((star) => (
        <button
          key={star}
          type="button"
          className={`star-btn ${star <= (hoverRating || value) ? 'filled' : 'empty'}`}
          onClick={() => handleClick(star)}
          onMouseEnter={() => handleMouseEnter(star)}
          onMouseLeave={handleMouseLeave}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
        </button>
      ))}
      <span className="rating-label">
        {value > 0 ? `${value} Star${value !== 1 ? 's' : ''}` : 'Select a rating'}
      </span>
    </div>
  );
};

export default function CreateReview({
  onSubmit,
  onCancel,
  isSubmitting = false
}) {
  const [rating, setRating] = useState(0);
  const [comment, setComment] = useState('');

  const characterCount = comment.length;
  const isValid = rating > 0 && comment.trim().length >= 10;

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!isValid) return;

    onSubmit?.({
      rating,
      comment: comment.trim()
    });

    setRating(0);
    setComment('');
  };

  const handleCommentChange = (e) => {
    const text = e.target.value;
    if (text.length <= MAX_CHARACTERS) {
      setComment(text);
    }
  };

  return (
    <div className="create-review-component">
      <h3>Write a Review</h3>
      <form onSubmit={handleSubmit}>
        <div className="form-group">
          <label>Your Rating</label>
          <StarRatingInput value={rating} onChange={setRating} />
        </div>

        <div className="form-group">
          <label htmlFor="review-comment">Your Review</label>
          <textarea
            id="review-comment"
            value={comment}
            onChange={handleCommentChange}
            placeholder="Share your experience with this game..."
            rows={5}
            className="review-textarea"
          />
          <div className="character-count">
            <span className={characterCount < 10 ? 'warning' : ''}>
              {characterCount < 10
                ? `Minimum 10 characters (${characterCount}/${MAX_CHARACTERS})`
                : `${characterCount}/${MAX_CHARACTERS}`}
            </span>
          </div>
        </div>

        <div className="form-actions">
          {onCancel && (
            <Button
              type="button"
              variant="ghost"
              onClick={onCancel}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
          )}
          <Button
            type="submit"
            variant="primary"
            disabled={!isValid || isSubmitting}
            loading={isSubmitting}
          >
            Submit Review
          </Button>
        </div>
      </form>
    </div>
  );
}