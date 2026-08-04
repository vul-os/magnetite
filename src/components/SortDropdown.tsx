import { useRef, useEffect } from 'react';
import { ChevronDownIcon, CheckIcon } from '../assets/icons';
import './SortDropdown.css';

const sortOptions = [
  { value: 'popular', label: 'Most Popular' },
  { value: 'newest', label: 'Newest' },
  { value: 'price-low', label: 'Price: Low to High' },
  { value: 'price-high', label: 'Price: High to Low' },
  { value: 'rating', label: 'Highest Rated' },
];

export default function SortDropdown({ value, onChange, onClose }) {
  const ref = useRef(null);

  useEffect(() => {
    function handleClickOutside(event) {
      if (ref.current && !ref.current.contains(event.target)) {
        onClose?.();
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  return (
    <div className="sort-dropdown" ref={ref}>
      <div className="sort-dropdown-header">
        <h4>Sort By</h4>
        <ChevronDownIcon className="sort-header-icon" />
      </div>
      <div className="sort-options">
        {sortOptions.map((option) => (
          <button
            key={option.value}
            className={`sort-option ${value === option.value ? 'active' : ''}`}
            onClick={() => onChange(option.value)}
          >
            <span className="sort-option-label">{option.label}</span>
            {value === option.value && <CheckIcon className="sort-check-icon" />}
          </button>
        ))}
      </div>
    </div>
  );
}
