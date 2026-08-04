import { useRef, useEffect } from 'react';
import {
  GameIcon,
  TrophyIcon,
  UsersIcon,
  CodeIcon,
  GlobeIcon,
  ChartIcon,
  CloseIcon,
  CheckIcon,
} from '../assets/icons';
import './CategoryFilter.css';

const categoryIcons: Record<string, typeof GameIcon> = {
  Action: GameIcon,
  Adventure: GlobeIcon,
  Strategy: TrophyIcon,
  RPG: UsersIcon,
  Simulation: CodeIcon,
  Sports: ChartIcon,
};

export interface CategoryFilterProps {
  categories?: string[];
  selectedCategories?: string[];
  onCategoryChange: (category: string) => void;
  onClose?: () => void;
}

export default function CategoryFilter({
  categories = [],
  selectedCategories = [],
  onCategoryChange,
  onClose,
}: CategoryFilterProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        onClose?.();
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  return (
    <div className="category-filter" ref={ref}>
      <div className="category-filter-header">
        <h4>Categories</h4>
        <button className="category-close-btn" onClick={onClose} aria-label="Close">
          <CloseIcon />
        </button>
      </div>
      <div className="category-list">
        {categories.length > 0 ? (
          categories.map((category) => {
            const Icon = categoryIcons[category] || GameIcon;
            const isSelected = selectedCategories.includes(category);
            return (
              <label key={category} className="category-item">
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => onCategoryChange(category)}
                  className="category-checkbox"
                />
                <span className={`category-check ${isSelected ? 'checked' : ''}`}>
                  {isSelected && <CheckIcon className="check-icon" />}
                </span>
                <Icon className="category-icon" />
                <span className="category-name">{category}</span>
              </label>
            );
          })
        ) : (
          <div className="category-empty">No categories available</div>
        )}
      </div>
    </div>
  );
}
