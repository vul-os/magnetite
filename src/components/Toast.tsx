import { useEffect, useState, useCallback, type ReactNode } from 'react';
import type { ToastType } from '../context/ToastContext';
import './Toast.css';

/* Broader than context/ToastContext's `Toast` (id/message/type only) — this
   component also supports an optional title and a custom auto-dismiss
   duration, matching how it was already used before the migration. */
export interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
  title?: string;
  duration?: number;
}

interface ToastProps {
  toast: ToastItem;
  onRemove: (id: number) => void;
  position?: string;
}

const icons: Record<ToastType, ReactNode> = {
  success: (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
      <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="2"/>
      <path d="M6 10l3 3 5-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  error: (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
      <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="2"/>
      <path d="M7 7l6 6M13 7l-6 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
  ),
  warning: (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
      <path d="M10 2L1 18h18L10 2z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
      <path d="M10 8v4M10 14v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
  ),
  info: (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
      <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="2"/>
      <path d="M10 9v5M10 6v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
  ),
};

function Toast({ toast, onRemove, position = 'top-right' }: ToastProps) {
  const [isExiting, setIsExiting] = useState(false);
  const [isEntering, setIsEntering] = useState(true);
  const [progress, setProgress] = useState(100);

  const handleClose = useCallback(() => {
    setIsExiting(true);
    setTimeout(() => onRemove(toast.id), 300);
  }, [onRemove, toast.id]);

  useEffect(() => {
    const startTime = Date.now();
    const duration = toast.duration || 5000;

    if (duration === 0) return;

    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 100 - (elapsed / duration) * 100);
      setProgress(remaining);
      if (remaining === 0) {
        clearInterval(interval);
        handleClose();
      }
    }, 50);

    return () => clearInterval(interval);
  }, [toast.duration, handleClose]);

  useEffect(() => {
    const enterTimer = setTimeout(() => setIsEntering(false), 20);
    return () => clearTimeout(enterTimer);
  }, []);

  return (
    <div
      className={`toast toast-${toast.type} toast-${position} ${isExiting ? 'toast-exit' : ''} ${isEntering ? 'toast-enter' : ''}`}
      role={toast.type === 'error' ? 'alert' : 'status'}
      aria-live={toast.type === 'error' ? 'assertive' : 'polite'}
      aria-atomic="true"
    >
      <div className="toast-icon" aria-hidden="true">{icons[toast.type]}</div>
      <div className="toast-content">
        {toast.title && <div className="toast-title">{toast.title}</div>}
        <div className="toast-message">{toast.message}</div>
      </div>
      <button className="toast-close" onClick={handleClose} aria-label="Close notification">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
        </svg>
      </button>
      {toast.duration !== 0 && (
        <div className="toast-progress" aria-hidden="true">
          <div className="toast-progress-bar" style={{ width: `${progress}%` }} />
        </div>
      )}
    </div>
  );
}

interface ToastContainerProps {
  toasts: ToastItem[];
  removeToast: (id: number) => void;
  position?: string;
}

export function ToastContainer({ toasts, removeToast, position = 'top-right' }: ToastContainerProps) {
  return (
    <div
      className={`toast-container toast-container-${position}`}
      aria-live="polite"
      aria-atomic="false"
      aria-label="Notifications"
      role="region"
    >
      {toasts.map(toast => (
        <Toast
          key={toast.id}
          toast={toast}
          onRemove={removeToast}
          position={position}
        />
      ))}
    </div>
  );
}

export default Toast;
