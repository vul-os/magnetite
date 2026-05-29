import { useState, useMemo } from 'react';
import Modal from './Modal';
import Button from './common/Button';
import './Modal.css';

export default function SelectModal({
  isOpen,
  onClose,
  title,
  options = [],
  value,
  onChange,
  placeholder = 'Search...',
  emptyMessage = 'No items found',
  size = 'md',
}) {
  const [search, setSearch] = useState('');

  const filteredOptions = useMemo(() => {
    if (!search) return options;
    const lowerSearch = search.toLowerCase();
    return options.filter(
      (option) =>
        option.label.toLowerCase().includes(lowerSearch) ||
        (option.description && option.description.toLowerCase().includes(lowerSearch))
    );
  }, [options, search]);

  const handleSelect = (option) => {
    onChange(option.value ?? option);
    onClose();
  };

  const handleClose = () => {
    setSearch('');
    onClose();
  };

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title={title} size={size}>
      <div className="select-modal">
        <div className="select-search">
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
            <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" />
            <path d="M13 13l3 3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={placeholder}
            className="select-search-input"
            autoComplete="off"
          />
          {search && (
            <button
              className="select-search-clear"
              onClick={() => setSearch('')}
              aria-label="Clear search"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M3 3l8 8M11 3l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
          )}
        </div>

        <div className="select-list" role="listbox">
          {filteredOptions.length === 0 ? (
            <div className="select-empty">{emptyMessage}</div>
          ) : (
            filteredOptions.map((option) => {
              const isSelected = value === (option.value ?? option);
              return (
                <button
                  key={option.value ?? option}
                  className={`select-item ${isSelected ? 'select-item-selected' : ''}`}
                  onClick={() => handleSelect(option)}
                  role="option"
                  aria-selected={isSelected}
                >
                  <div className="select-item-content">
                    <span className="select-item-label">{option.label}</span>
                    {option.description && (
                      <span className="select-item-description">{option.description}</span>
                    )}
                  </div>
                  {isSelected && (
                    <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                      <path
                        d="M4 9l4 4 6-7"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  )}
                </button>
              );
            })
          )}
        </div>

        {value !== undefined && (
          <div className="select-footer">
            <span className="select-selected-label">Selected:</span>
            <span className="select-selected-value">
              {options.find((o) => (o.value ?? o) === value)?.label ?? value}
            </span>
          </div>
        )}
      </div>
    </Modal>
  );
}
