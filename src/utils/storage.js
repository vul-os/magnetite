const isServer = typeof window === 'undefined';

export const storage = {
  get(key, defaultValue = null) {
    if (isServer) return defaultValue;
    try {
      const item = localStorage.getItem(key);
      return item ? JSON.parse(item) : defaultValue;
    } catch {
      return defaultValue;
    }
  },

  set(key, value) {
    if (isServer) return;
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      console.warn('Failed to save to localStorage');
    }
  },

  remove(key) {
    if (isServer) return;
    localStorage.removeItem(key);
  },

  clear() {
    if (isServer) return;
    localStorage.clear();
  },
};

export const sessionStorage = {
  get(key, defaultValue = null) {
    if (isServer) return defaultValue;
    try {
      const item = window.sessionStorage.getItem(key);
      return item ? JSON.parse(item) : defaultValue;
    } catch {
      return defaultValue;
    }
  },

  set(key, value) {
    if (isServer) return;
    try {
      window.sessionStorage.setItem(key, JSON.stringify(value));
    } catch {
      console.warn('Failed to save to sessionStorage');
    }
  },

  remove(key) {
    if (isServer) return;
    window.sessionStorage.removeItem(key);
  },
};
