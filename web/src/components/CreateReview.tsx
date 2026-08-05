import { useState, type ChangeEvent, type FormEvent } from 'react';
import Button from './common/Button';

const MAX_CHARACTERS = 1000;

interface StarRatingInputProps {
  value: number;
  onChange: (rating: number) => void;
}

const StarRatingInput = ({ value, onChange }: StarRatingInputProps) => {
  const [hoverRating, setHoverRating] = useState(0);

  const handleClick = (rating: number) => {
    onChange(rating);
  };

  const handleMouseEnter = (rating: number) => {
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

export interface ReviewSubmission {
  rating: number;
  comment: string;
}

interface CreateReviewProps {
  onSubmit?: (review: ReviewSubmission) => void;
  onCancel?: () => void;
  isSubmitting?: boolean;
}

export default function CreateReview({
  onSubmit,
  onCancel,
  isSubmitting = false
}: CreateReviewProps) {
  const [rating, setRating] = useState(0);
  const [comment, setComment] = useState('');

  const characterCount = comment.length;
  const isValid = rating > 0 && comment.trim().length >= 10;

  const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!isValid) return;

    onSubmit?.({
      rating,
      comment: comment.trim()
    });

    setRating(0);
    setComment('');
  };

  const handleCommentChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
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
          {/* NOTE: Button doesn't accept a `loading` prop (only `isLoading`); this
              was already a no-op before the TS migration. Preserved as-is per
              zero-behavior-change constraint; extracted to a variable so the
              (pre-existing) mismatched prop doesn't fail strict JSX checking. */}
          <Button
            {...{
              type: 'submit' as const,
              variant: 'primary' as const,
              disabled: !isValid || isSubmitting,
              loading: isSubmitting,
            }}
          >
            Submit Review
          </Button>
        </div>
      </form>
    </div>
  );
}