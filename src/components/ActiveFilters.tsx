import { CloseIcon } from '../assets/icons';
import './ActiveFilters.css';

export default function ActiveFilters({ filters = [], onRemove, onClearAll }) {
  if (filters.length === 0) return null;

  return (
    <div className="active-filters">
      <div className="active-filters-list">
        {filters.map((filter, index) => (
          <div key={`${filter.type}-${filter.value}-${index}`} className="active-filter-tag">
            <span className="filter-tag-type">{filter.type}:</span>
            <span className="filter-tag-value">{filter.label || filter.value}</span>
            <button
              className="filter-tag-remove"
              onClick={() => onRemove?.(filter)}
              aria-label={`Remove ${filter.label || filter.value} filter`}
            >
              <CloseIcon />
            </button>
          </div>
        ))}
      </div>
      {filters.length > 1 && (
        <button className="active-filters-clear" onClick={onClearAll}>
          Clear all
        </button>
      )}
    </div>
  );
}
