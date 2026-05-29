import { useState, useRef, useEffect, useMemo } from 'react';
import { ChevronDownIcon, SearchIcon, CloseIcon, CheckIcon } from '../../assets/icons';
import './Select.css';

export default function Select({
  options = [],
  value,
  onChange,
  placeholder = 'Select...',
  isSearchable = false,
  isMulti = false,
  isClearable = false,
  disabled = false,
  error = '',
  className = '',
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [dropdownPosition, setDropdownPosition] = useState('bottom');
  const containerRef = useRef(null);
  const dropdownRef = useRef(null);
  const searchInputRef = useRef(null);

  const selectedValues = isMulti ? (Array.isArray(value) ? value : []) : value;

  const filteredOptions = useMemo(() => {
    if (!searchTerm) return options;
    const term = searchTerm.toLowerCase();
    return options.filter((opt) => opt.label.toLowerCase().includes(term));
  }, [options, searchTerm]);

  const groupedOptions = useMemo(() => {
    const groups = {};
    filteredOptions.forEach((opt) => {
      const groupKey = opt.group || '';
      if (!groups[groupKey]) groups[groupKey] = [];
      groups[groupKey].push(opt);
    });
    return groups;
  }, [filteredOptions]);

  useEffect(() => {
    function handleClickOutside(event) {
      if (containerRef.current && !containerRef.current.contains(event.target)) {
        setIsOpen(false);
        setSearchTerm('');
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen && isSearchable && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen, isSearchable]);

  useEffect(() => {
    if (isOpen && dropdownRef.current && containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const spaceBelow = window.innerHeight - rect.bottom;
      const spaceAbove = rect.top;
      const dropdownHeight = Math.min(dropdownRef.current.scrollHeight, 300);

      if (spaceBelow < dropdownHeight && spaceAbove > spaceBelow) {
        setDropdownPosition('top');
      } else {
        setDropdownPosition('bottom');
      }
    }
  }, [isOpen]);

  const handleSelect = (optionValue) => {
    if (isMulti) {
      const newValue = selectedValues.includes(optionValue)
        ? selectedValues.filter((v) => v !== optionValue)
        : [...selectedValues, optionValue];
      onChange(newValue);
    } else {
      onChange(optionValue);
      setIsOpen(false);
      setSearchTerm('');
    }
  };

  const handleClear = (e) => {
    e.stopPropagation();
    onChange(isMulti ? [] : '');
  };

  const getDisplayValue = () => {
    if (isMulti) {
      if (selectedValues.length === 0) return placeholder;
      if (selectedValues.length === 1) {
        const opt = options.find((o) => o.value === selectedValues[0]);
        return opt?.label || selectedValues[0];
      }
      return `${selectedValues.length} selected`;
    }
    if (!value) return placeholder;
    const opt = options.find((o) => o.value === value);
    return opt?.label || value;
  };

  const isSelected = (optValue) => {
    if (isMulti) {
      return selectedValues.includes(optValue);
    }
    return value === optValue;
  };

  const containerClasses = [
    'select-container',
    className,
    isOpen && 'select-open',
    disabled && 'select-disabled',
    error && 'select-error',
  ].filter(Boolean).join(' ');

  return (
    <div className={containerClasses} ref={containerRef}>
      <button
        type="button"
        className="select-trigger"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
      >
        <span className={`select-value ${!value || (isMulti && selectedValues.length === 0) ? 'select-placeholder' : ''}`}>
          {getDisplayValue()}
        </span>
        <span className="select-icons">
          {isClearable && (isMulti ? selectedValues.length > 0 : value) && (
            <span className="select-clear" onClick={handleClear}>
              <CloseIcon />
            </span>
          )}
          <ChevronDownIcon className={`select-chevron ${isOpen ? 'select-chevron-open' : ''}`} />
        </span>
      </button>

      {isOpen && (
        <div
          ref={dropdownRef}
          className={`select-dropdown select-dropdown-${dropdownPosition}`}
        >
          {isSearchable && (
            <div className="select-search">
              <SearchIcon className="select-search-icon" />
              <input
                ref={searchInputRef}
                type="text"
                className="select-search-input"
                placeholder="Search..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
              />
            </div>
          )}

          <div className="select-options">
            {Object.keys(groupedOptions).length === 0 ? (
              <div className="select-no-results">No options found</div>
            ) : (
              Object.entries(groupedOptions).map(([group, opts]) => (
                <div key={group} className="select-group">
                  {group && <div className="select-group-label">{group}</div>}
                  <div className="select-group-options">
                    {opts.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        className={`select-option ${isSelected(option.value) ? 'select-option-selected' : ''}`}
                        onClick={() => handleSelect(option.value)}
                      >
                        <span className="select-option-label">{option.label}</span>
                        {isSelected(option.value) && (
                          <CheckIcon className="select-option-check" />
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {error && <span className="select-error-text">{error}</span>}
    </div>
  );
}
