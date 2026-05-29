import { createContext, useContext, useState, useCallback } from 'react';

const ToastContext = createContext();

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);

  const addToast = useCallback((message, type = 'info') => {
    const id = Date.now() + Math.random();
    setToasts(prev => {
      const newToasts = [...prev, { id, message, type }];
      return newToasts.slice(-5);
    });
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 5000);
  }, []);

  const removeToast = useCallback((id) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast }}>
      {children}
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) throw new Error('useToast must be used within ToastProvider');
  return {
    toasts: context.toasts,
    addToast: context.addToast,
    removeToast: context.removeToast,
    success: (message) => context.addToast(message, 'success'),
    error: (message) => context.addToast(message, 'error'),
    warning: (message) => context.addToast(message, 'warning'),
    info: (message) => context.addToast(message, 'info'),
  };
}
