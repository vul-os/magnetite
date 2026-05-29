import { useMemo } from 'react';
import Modal from './common/Modal';
import './KeyboardShortcuts.css';

const shortcutCategories = {
  navigation: {
    label: 'Navigation',
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <polygon points="3 11 22 2 13 21 11 13 3 11" />
      </svg>
    ),
  },
  actions: {
    label: 'Actions',
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="10" />
        <path d="M12 16v-4M12 8h.01" />
      </svg>
    ),
  },
};

const defaultShortcuts = [
  { key: '?', description: 'Show shortcuts', category: 'actions', allowInInput: true },
  { key: 'k', description: 'Open search', category: 'actions', allowInInput: true },
  { key: 'g h', description: 'Go to home', category: 'navigation' },
  { key: 'g m', description: 'Go to marketplace', category: 'navigation' },
  { key: 'g d', description: 'Go to developer dashboard', category: 'navigation' },
  { key: 'c', description: 'Open wallet', category: 'actions' },
  { key: 'Esc', description: 'Close modal', category: 'actions', allowInInput: true },
];

export default function KeyboardShortcuts({ isOpen, onClose, shortcuts = defaultShortcuts }) {
  const groupedShortcuts = useMemo(() => {
    const groups = {};
    shortcuts.forEach(shortcut => {
      const category = shortcut.category || 'actions';
      if (!groups[category]) {
        groups[category] = [];
      }
      groups[category].push(shortcut);
    });
    return groups;
  }, [shortcuts]);

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Keyboard Shortcuts" size="md">
      <div className="keyboard-shortcuts">
        {Object.entries(groupedShortcuts).map(([categoryKey, categoryShortcuts]) => {
          const category = shortcutCategories[categoryKey] || { label: categoryKey, icon: null };
          return (
            <div key={categoryKey} className="shortcut-category">
              <div className="shortcut-category-header">
                {category.icon}
                <span>{category.label}</span>
              </div>
              <div className="shortcut-list">
                {categoryShortcuts.map((shortcut, index) => (
                  <div key={`${shortcut.key}-${index}`} className="shortcut-item">
                    <div className="shortcut-keys">
                      {shortcut.key.split(' ').map((k, i) => (
                        <kbd key={i} className="shortcut-key">{k}</kbd>
                      ))}
                    </div>
                    <span className="shortcut-description">{shortcut.description}</span>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </Modal>
  );
}