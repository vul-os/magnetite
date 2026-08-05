const isServer = typeof window === 'undefined';

export const storage = {
  get<T = unknown>(key: string, defaultValue: T | null = null): T | null {
    if (isServer) return defaultValue;
    try {
      const item = localStorage.getItem(key);
      return item ? (JSON.parse(item) as T) : defaultValue;
    } catch {
      return defaultValue;
    }
  },

  set(key: string, value: unknown): void {
    if (isServer) return;
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      console.warn('Failed to save to localStorage');
    }
  },

  remove(key: string): void {
    if (isServer) return;
    localStorage.removeItem(key);
  },

  clear(): void {
    if (isServer) return;
    localStorage.clear();
  },
};

export const sessionStorage = {
  get<T = unknown>(key: string, defaultValue: T | null = null): T | null {
    if (isServer) return defaultValue;
    try {
      const item = window.sessionStorage.getItem(key);
      return item ? (JSON.parse(item) as T) : defaultValue;
    } catch {
      return defaultValue;
    }
  },

  set(key: string, value: unknown): void {
    if (isServer) return;
    try {
      window.sessionStorage.setItem(key, JSON.stringify(value));
    } catch {
      console.warn('Failed to save to sessionStorage');
    }
  },

  remove(key: string): void {
    if (isServer) return;
    window.sessionStorage.removeItem(key);
  },
};
