import type { ReactNode } from 'react';
import Modal from './Modal';
import Button from './Button';
import { useTranslation } from '../../i18n/useTranslation';
import './ConfirmDialog.css';

export type ConfirmDialogVariant = 'danger' | 'warning' | 'info';

const variantClasses: Record<ConfirmDialogVariant, string> = {
  danger: 'confirm-dialog-danger',
  warning: 'confirm-dialog-warning',
  info: 'confirm-dialog-info',
};

const icons: Record<ConfirmDialogVariant, ReactNode> = {
  danger: (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" />
      <path d="M12 7v6M12 16v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  ),
  warning: (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
      <path d="M12 3L2 21h20L12 3z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
      <path d="M12 10v4M12 17v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  ),
  info: (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" />
      <path d="M12 10v5M12 7v1" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  ),
};

export interface ConfirmDialogProps {
  isOpen: boolean;
  onClose?: () => void;
  onConfirm?: () => void;
  onCancel?: () => void;
  title?: string | undefined;
  message?: ReactNode;
  confirmText?: string;
  cancelText?: string;
  variant?: ConfirmDialogVariant;
}

export default function ConfirmDialog({
  isOpen,
  onClose,
  onConfirm,
  onCancel,
  title,
  message,
  confirmText,
  cancelText,
  variant = 'info',
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  const resolvedConfirmText = confirmText ?? t('common.confirm');
  const resolvedCancelText = cancelText ?? t('common.cancel');

  // Modal calls its onClose unconditionally on Escape and on the header close button
  // (closeOnBackdrop only guards the backdrop click). ConfirmDialogContext never passes
  // onClose, only onCancel — so onClose was undefined and Escape/close-button dismissal
  // threw "onClose is not a function", crashing whatever screen the dialog was open on.
  // Dismissing via Escape or the close button is the same user intent as clicking
  // Cancel, so fall back to onCancel; a final no-op keeps the handler always callable
  // if a future caller supplies neither.
  const handleClose = onClose ?? onCancel ?? (() => {});

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title={title} size="sm" closeOnBackdrop={false}>
      <div className={`confirm-dialog-body ${variantClasses[variant]}`}>
        <div className="confirm-dialog-icon" aria-hidden="true">{icons[variant]}</div>
        <p className="confirm-dialog-message">{message}</p>
      </div>
      <div className="confirm-dialog-actions">
        <Button variant="secondary" onClick={onCancel}>
          {resolvedCancelText}
        </Button>
        <Button variant={variant === 'danger' ? 'danger' : 'primary'} onClick={onConfirm}>
          {resolvedConfirmText}
        </Button>
      </div>
    </Modal>
  );
}
