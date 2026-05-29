import { useRef, useEffect, useState } from 'react';
import './Tabs.css';

export default function Tabs({
  tabs = [],
  activeTab,
  onChange,
  variant = 'underline',
  orientation = 'horizontal',
  children,
  className = '',
}) {
  const tabsListRef = useRef(null);
  const [indicatorStyle, setIndicatorStyle] = useState({});
  const [_focusedIndex, setFocusedIndex] = useState(-1);

  useEffect(() => {
    updateIndicator();
  }, [activeTab, orientation]);

  const updateIndicator = () => {
    if (!tabsListRef.current) return;
    const activeButton = tabsListRef.current.querySelector(`[data-tab-id="${activeTab}"]`);
    if (!activeButton) return;

    if (orientation === 'horizontal') {
      setIndicatorStyle({
        left: activeButton.offsetLeft,
        width: activeButton.offsetWidth,
      });
    } else {
      setIndicatorStyle({
        top: activeButton.offsetTop,
        height: activeButton.offsetHeight,
      });
    }
  };

  const handleKeyDown = (e) => {
    const currentIndex = tabs.findIndex((t) => t.id === activeTab);
    let newIndex;

    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      newIndex = (currentIndex + 1) % tabs.length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      newIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    } else if (e.key === 'Home') {
      e.preventDefault();
      newIndex = 0;
    } else if (e.key === 'End') {
      e.preventDefault();
      newIndex = tabs.length - 1;
    } else {
      return;
    }

    onChange(tabs[newIndex].id);
    setFocusedIndex(newIndex);
  };

  const handleFocus = (index) => {
    setFocusedIndex(index);
  };

  const activeIndex = tabs.findIndex((t) => t.id === activeTab);

  return (
    <div
      className={`tabs tabs-${variant} tabs-${orientation} ${className}`}
      data-orientation={orientation}
    >
      <div
        ref={tabsListRef}
        className="tabs-list"
        role="tablist"
        aria-orientation={orientation}
        onKeyDown={handleKeyDown}
      >
        <div
          className="tabs-indicator"
          data-orientation={orientation}
          style={
            orientation === 'horizontal'
              ? { left: indicatorStyle.left, width: indicatorStyle.width }
              : { top: indicatorStyle.top, height: indicatorStyle.height }
          }
        />
        {tabs.map((tab, index) => (
          <button
            key={tab.id}
            role="tab"
            id={`tab-${tab.id}`}
            aria-selected={activeTab === tab.id}
            aria-controls={`panel-${tab.id}`}
            tabIndex={activeTab === tab.id ? 0 : -1}
            data-tab-id={tab.id}
            className={`tabs-trigger ${activeTab === tab.id ? 'tabs-trigger-active' : ''}`}
            onClick={() => onChange(tab.id)}
            onFocus={() => handleFocus(index)}
          >
            {tab.icon && <span className="tabs-icon">{tab.icon}</span>}
            <span className="tabs-label">{tab.label}</span>
            {tab.badge !== undefined && (
              <span className={`tabs-badge ${activeTab === tab.id ? 'tabs-badge-active' : ''}`}>
                {tab.badge}
              </span>
            )}
          </button>
        ))}
      </div>

      {children && (
        <div className="tabs-content-wrapper">
          {children.map((child, index) => (
            <div
              key={tabs[index]?.id || index}
              role="tabpanel"
              id={`panel-${tabs[index]?.id}`}
              aria-labelledby={`tab-${tabs[index]?.id}`}
              tabIndex={0}
              className={`tabs-content ${activeIndex === index ? 'tabs-content-active' : ''}`}
              hidden={activeIndex !== index}
            >
              {child}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
