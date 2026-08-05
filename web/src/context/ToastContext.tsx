import { createContext, useContext, useState, useCallback, type ReactNode } from 'react';

export type ToastType = 'info' | 'success' | 'error' | 'warning';

export interface Toast {
  id: number;
  message: string;
  type: ToastType;
}

export interface ToastContextValue {
  toasts: Toast[];
  addToast: (message: string, type?: ToastType) => void;
  removeToast: (id: number) => void;
}

const ToastContext = createContext<ToastContextValue | undefined>(undefined);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback((message: string, type: ToastType = 'info') => {
    const id = Date.now() + Math.random();
    setToasts(prev => {
      const newToasts = [...prev, { id, message, type }];
      return newToasts.slice(-5);
    });
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 5000);
  }, []);

  const removeToast = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast }}>
      {children}
    </ToastContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useToast() {
  const context = useContext(ToastContext);
  if (!context) throw new Error('useToast must be used within ToastProvider');
  return {
    toasts: context.toasts,
    addToast: context.addToast,
    removeToast: context.removeToast,
    success: (message: string) => context.addToast(message, 'success'),
    error: (message: string) => context.addToast(message, 'error'),
    warning: (message: string) => context.addToast(message, 'warning'),
    info: (message: string) => context.addToast(message, 'info'),
  };
}
