import { useState, useRef, useEffect, useMemo, useId } from 'react';
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
  label,
  id,
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [dropdownPosition, setDropdownPosition] = useState('bottom');
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const containerRef = useRef(null);
  const dropdownRef = useRef(null);
  const searchInputRef = useRef(null);
  const uid = useId();
  const selectId = id || uid;
  const listboxId = `${selectId}-listbox`;

  const selectedValues = isMulti ? (Array.isArray(value) ? value : []) : value;

  const filteredOptions = useMemo(() => {
    if (!searchTerm) return options;
    const term = searchTerm.toLowerCase();
    return options.filter((opt) => opt.label.toLowerCase().includes(term));
  }, [options, searchTerm]);

  const flatFilteredOptions = useMemo(() => {
    return filteredOptions;
  }, [filteredOptions]);

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
        setFocusedIndex(-1);
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

  // Scroll focused option into view
  useEffect(() => {
    if (!isOpen || focusedIndex < 0 || !dropdownRef.current) return;
    const focused = dropdownRef.current.querySelector(`[data-option-index="${focusedIndex}"]`);
    focused?.scrollIntoView({ block: 'nearest' });
  }, [focusedIndex, isOpen]);

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
      setFocusedIndex(-1);
    }
  };

  const handleClear = (e) => {
    e.stopPropagation();
    onChange(isMulti ? [] : '');
  };

  const handleTriggerKeyDown = (e) => {
    if (disabled) return;
    switch (e.key) {
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (!isOpen) {
          setIsOpen(true);
          setFocusedIndex(0);
        } else if (focusedIndex >= 0 && flatFilteredOptions[focusedIndex]) {
          handleSelect(flatFilteredOptions[focusedIndex].value);
        }
        break;
      case 'ArrowDown':
        e.preventDefault();
        if (!isOpen) {
          setIsOpen(true);
          setFocusedIndex(0);
        } else {
          setFocusedIndex(i => Math.min(i + 1, flatFilteredOptions.length - 1));
        }
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (isOpen) {
          setFocusedIndex(i => Math.max(i - 1, 0));
        }
        break;
      case 'Escape':
        e.preventDefault();
        setIsOpen(false);
        setSearchTerm('');
        setFocusedIndex(-1);
        break;
      case 'Tab':
        setIsOpen(false);
        setSearchTerm('');
        setFocusedIndex(-1);
        break;
    }
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

  const activeFlatIndex = focusedIndex >= 0 && flatFilteredOptions[focusedIndex]
    ? `${selectId}-option-${focusedIndex}`
    : undefined;

  const containerClasses = [
    'select-container',
    className,
    isOpen && 'select-open',
    disabled && 'select-disabled',
    error && 'select-error',
  ].filter(Boolean).join(' ');

  let optionGlobalIndex = -1;

  return (
    <div className={containerClasses} ref={containerRef}>
      {label && (
        <label htmlFor={selectId} className="select-label">
          {label}
        </label>
      )}
      <button
        type="button"
        id={selectId}
        className="select-trigger"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        onKeyDown={handleTriggerKeyDown}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={isOpen ? listboxId : undefined}
        aria-activedescendant={isOpen ? activeFlatIndex : undefined}
        aria-invalid={error ? 'true' : undefined}
      >
        <span className={`select-value ${!value || (isMulti && selectedValues.length === 0) ? 'select-placeholder' : ''}`}>
          {getDisplayValue()}
        </span>
        <span className="select-icons" aria-hidden="true">
          {isClearable && (isMulti ? selectedValues.length > 0 : value) && (
            <span
              className="select-clear"
              onClick={handleClear}
              role="button"
              tabIndex={0}
              aria-label="Clear selection"
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleClear(e); } }}
            >
              <CloseIcon />
            </span>
          )}
          <ChevronDownIcon className={`select-chevron ${isOpen ? 'select-chevron-open' : ''}`} />
        </span>
      </button>

      {isOpen && (
        <div
          ref={dropdownRef}
          id={listboxId}
          role="listbox"
          aria-multiselectable={isMulti || undefined}
          className={`select-dropdown select-dropdown-${dropdownPosition}`}
        >
          {isSearchable && (
            <div className="select-search">
              <SearchIcon className="select-search-icon" aria-hidden="true" />
              <input
                ref={searchInputRef}
                type="text"
                className="select-search-input"
                placeholder="Search..."
                value={searchTerm}
                onChange={(e) => { setSearchTerm(e.target.value); setFocusedIndex(0); }}
                aria-label="Search options"
                aria-autocomplete="list"
                aria-controls={listboxId}
              />
            </div>
          )}

          <div className="select-options">
            {Object.keys(groupedOptions).length === 0 ? (
              <div className="select-no-results" role="option" aria-disabled="true">No options found</div>
            ) : (
              Object.entries(groupedOptions).map(([group, opts]) => (
                <div key={group} className="select-group">
                  {group && (
                    <div className="select-group-label" role="presentation">{group}</div>
                  )}
                  <div className="select-group-options">
                    {opts.map((option) => {
                      optionGlobalIndex += 1;
                      const idx = optionGlobalIndex;
                      const isFocused = idx === focusedIndex;
                      return (
                        <button
                          key={option.value}
                          id={`${selectId}-option-${idx}`}
                          type="button"
                          role="option"
                          aria-selected={isSelected(option.value)}
                          data-option-index={idx}
                          className={`select-option ${isSelected(option.value) ? 'select-option-selected' : ''} ${isFocused ? 'select-option-focused' : ''}`}
                          onClick={() => handleSelect(option.value)}
                          onMouseEnter={() => setFocusedIndex(idx)}
                          tabIndex={-1}
                        >
                          <span className="select-option-label">{option.label}</span>
                          {isSelected(option.value) && (
                            <CheckIcon className="select-option-check" aria-hidden="true" />
                          )}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {error && <span className="select-error-text" role="alert">{error}</span>}
    </div>
  );
}
