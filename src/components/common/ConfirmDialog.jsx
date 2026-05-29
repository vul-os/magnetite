import Modal from './Modal';
import Button from './Button';
import './ConfirmDialog.css';

const variantClasses = {
  danger: 'confirm-dialog-danger',
  warning: 'confirm-dialog-warning',
  info: 'confirm-dialog-info',
};

const icons = {
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

export default function ConfirmDialog({
  isOpen,
  onClose,
  onConfirm,
  onCancel,
  title,
  message,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'info',
}) {
  return (
    <Modal isOpen={isOpen} onClose={onClose} title={title} size="sm" closeOnBackdrop={false}>
      <div className={`confirm-dialog-body ${variantClasses[variant]}`}>
        <div className="confirm-dialog-icon">{icons[variant]}</div>
        <p className="confirm-dialog-message">{message}</p>
      </div>
      <div className="confirm-dialog-actions">
        <Button variant="secondary" onClick={onCancel}>
          {cancelText}
        </Button>
        <Button variant={variant === 'danger' ? 'danger' : 'primary'} onClick={onConfirm}>
          {confirmText}
        </Button>
      </div>
    </Modal>
  );
}
