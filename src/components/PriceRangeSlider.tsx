import { useState, useRef, useEffect } from 'react';
import { CloseIcon } from '../assets/icons';
import './PriceRangeSlider.css';

export default function PriceRangeSlider({
  min = 0,
  max = 100,
  value = { min: 0, max: 100 },
  onChange,
  onClose,
}) {
  const [localValue, setLocalValue] = useState(() => value);
  const ref = useRef(null);
  const initialValue = useRef(value);

  useEffect(() => {
    function handleClickOutside(event) {
      if (ref.current && !ref.current.contains(event.target)) {
        onChange?.(localValue);
        onClose?.();
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [localValue, onChange, onClose]);

  useEffect(() => {
    if (value.min !== initialValue.current.min || value.max !== initialValue.current.max) {
      setLocalValue(value);
      initialValue.current = value;
    }
  }, [value]);

  const handleMinChange = (e) => {
    const newMin = Math.min(Number(e.target.value), localValue.max - 1);
    setLocalValue((prev) => ({ ...prev, min: newMin }));
  };

  const handleMaxChange = (e) => {
    const newMax = Math.max(Number(e.target.value), localValue.min + 1);
    setLocalValue((prev) => ({ ...prev, max: newMax }));
  };

  const handleMinInputChange = (e) => {
    const val = Number(e.target.value);
    if (!isNaN(val) && val >= min && val < localValue.max) {
      setLocalValue((prev) => ({ ...prev, min: val }));
    }
  };

  const handleMaxInputChange = (e) => {
    const val = Number(e.target.value);
    if (!isNaN(val) && val <= max && val > localValue.min) {
      setLocalValue((prev) => ({ ...prev, max: val }));
    }
  };

  const minPercent = ((localValue.min - min) / (max - min)) * 100;
  const maxPercent = ((localValue.max - min) / (max - min)) * 100;

  return (
    <div className="price-range-slider" ref={ref}>
      <div className="price-range-header">
        <h4>Price Range</h4>
        <button className="price-close-btn" onClick={() => {
          onChange?.(localValue);
          onClose?.();
        }} aria-label="Close">
          <CloseIcon />
        </button>
      </div>

      <div className="price-range-content">
        <div className="price-inputs">
          <div className="price-input-group">
            <label>Min</label>
            <div className="price-input-wrapper">
              <span className="price-currency">$</span>
              <input
                type="number"
                value={localValue.min}
                onChange={handleMinInputChange}
                min={min}
                max={localValue.max - 1}
                className="price-input"
              />
            </div>
          </div>
          <span className="price-separator">—</span>
          <div className="price-input-group">
            <label>Max</label>
            <div className="price-input-wrapper">
              <span className="price-currency">$</span>
              <input
                type="number"
                value={localValue.max}
                onChange={handleMaxInputChange}
                min={localValue.min + 1}
                max={max}
                className="price-input"
              />
            </div>
          </div>
        </div>

        <div className="price-slider-container">
          <div className="price-slider-track">
            <div
              className="price-slider-range"
              style={{
                left: `${minPercent}%`,
                width: `${maxPercent - minPercent}%`,
              }}
            />
          </div>
          <input
            type="range"
            value={localValue.min}
            onChange={handleMinChange}
            min={min}
            max={max}
            className="price-slider price-slider-min"
          />
          <input
            type="range"
            value={localValue.max}
            onChange={handleMaxChange}
            min={min}
            max={max}
            className="price-slider price-slider-max"
          />
        </div>

        <div className="price-preview">
          <span className="price-preview-label">Selected range:</span>
          <span className="price-preview-value">
            ${localValue.min} - ${localValue.max}
          </span>
        </div>
      </div>
    </div>
  );
}
