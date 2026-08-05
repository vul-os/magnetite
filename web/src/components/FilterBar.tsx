import { useState, useRef, useEffect } from 'react';
import type { PriceRange } from './PriceRangeSlider';
import type { ActiveFilter } from './ActiveFilters';
import { ChevronDownIcon, CloseIcon } from '../assets/icons';
import CategoryFilter from './CategoryFilter';
import PriceRangeSlider from './PriceRangeSlider';
import SortDropdown from './SortDropdown';
import ActiveFilters from './ActiveFilters';
import './FilterBar.css';

type DropdownName = 'category' | 'price' | 'sort';
type OpenDropdownState = Partial<Record<DropdownName, boolean>> | null;

export interface FilterBarProps {
  categories?: string[];
  selectedCategories?: string[];
  onCategoriesChange?: (categories: string[]) => void;
  priceRange?: PriceRange;
  selectedPriceRange?: PriceRange;
  onPriceRangeChange?: (value: PriceRange) => void;
  sortBy?: string;
  onSortChange?: (value: string) => void;
  activeFilters?: ActiveFilter[];
  onRemoveFilter?: (filter: ActiveFilter) => void;
  onClearAll?: () => void;
}

export default function FilterBar({
  categories = [],
  selectedCategories = [],
  onCategoriesChange,
  priceRange = { min: 0, max: 100 },
  selectedPriceRange = { min: 0, max: 100 },
  onPriceRangeChange,
  sortBy = 'popular',
  onSortChange,
  activeFilters = [],
  onRemoveFilter,
  onClearAll,
}: FilterBarProps) {
  const [openDropdown, setOpenDropdown] = useState<OpenDropdownState>(null);
  const dropdownRefs = useRef<Partial<Record<DropdownName, HTMLDivElement | null>>>({});

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      Object.keys(openDropdown || {}).forEach((key) => {
        const dropdownKey = key as DropdownName;
        if (dropdownRefs.current[dropdownKey] && !dropdownRefs.current[dropdownKey].contains(event.target as Node)) {
          setOpenDropdown((prev) => ({ ...prev, [dropdownKey]: false }));
        }
      });
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [openDropdown]);

  const toggleDropdown = (name: DropdownName) => {
    setOpenDropdown((prev) => ({ ...prev, [name]: !(prev as Partial<Record<DropdownName, boolean>>)[name] }));
  };

  const handleCategoryChange = (category: string) => {
    const newCategories = selectedCategories.includes(category)
      ? selectedCategories.filter((c) => c !== category)
      : [...selectedCategories, category];
    onCategoriesChange?.(newCategories);
  };

  const hasActiveFilters =
    selectedCategories.length > 0 ||
    selectedPriceRange.min > priceRange.min ||
    selectedPriceRange.max < priceRange.max;

  return (
    <div className="filter-bar">
      <div className="filter-bar-main">
        <div className="filter-bar-left">
          <div className="filter-dropdown" ref={(el) => { dropdownRefs.current.category = el; }}>
            <button
              className={`filter-dropdown-trigger ${selectedCategories.length > 0 ? 'active' : ''}`}
              onClick={() => toggleDropdown('category')}
              aria-expanded={openDropdown?.category}
            >
              <span>Category</span>
              {selectedCategories.length > 0 && (
                <span className="filter-count">{selectedCategories.length}</span>
              )}
              <ChevronDownIcon
                className={`filter-chevron ${openDropdown?.category ? 'open' : ''}`}
              />
            </button>
            {openDropdown?.category && (
              <CategoryFilter
                categories={categories}
                selectedCategories={selectedCategories}
                onCategoryChange={handleCategoryChange}
                onClose={() => setOpenDropdown((prev) => ({ ...prev, category: false }))}
              />
            )}
          </div>

          <div className="filter-dropdown" ref={(el) => { dropdownRefs.current.price = el; }}>
            <button
              className={`filter-dropdown-trigger ${
                selectedPriceRange.min > priceRange.min || selectedPriceRange.max < priceRange.max
                  ? 'active'
                  : ''
              }`}
              onClick={() => toggleDropdown('price')}
              aria-expanded={openDropdown?.price}
            >
              <span>Price</span>
              <ChevronDownIcon
                className={`filter-chevron ${openDropdown?.price ? 'open' : ''}`}
              />
            </button>
            {openDropdown?.price && (
              <PriceRangeSlider
                min={priceRange.min}
                max={priceRange.max}
                value={selectedPriceRange}
                onChange={onPriceRangeChange}
                onClose={() => setOpenDropdown((prev) => ({ ...prev, price: false }))}
              />
            )}
          </div>

          <div className="filter-dropdown" ref={(el) => { dropdownRefs.current.sort = el; }}>
            <button
              className="filter-dropdown-trigger"
              onClick={() => toggleDropdown('sort')}
              aria-expanded={openDropdown?.sort}
            >
              <span>Sort: {sortBy}</span>
              <ChevronDownIcon
                className={`filter-chevron ${openDropdown?.sort ? 'open' : ''}`}
              />
            </button>
            {openDropdown?.sort && (
              <SortDropdown
                value={sortBy}
                onChange={(value) => {
                  onSortChange?.(value);
                  setOpenDropdown((prev) => ({ ...prev, sort: false }));
                }}
                onClose={() => setOpenDropdown((prev) => ({ ...prev, sort: false }))}
              />
            )}
          </div>
        </div>

        <div className="filter-bar-right">
          {hasActiveFilters && (
            <button className="clear-all-btn" onClick={onClearAll}>
              <CloseIcon className="clear-icon" />
              Clear all
            </button>
          )}
        </div>
      </div>

      {activeFilters.length > 0 && (
        <ActiveFilters
          filters={activeFilters}
          onRemove={onRemoveFilter}
          onClearAll={onClearAll}
        />
      )}
    </div>
  );
}
