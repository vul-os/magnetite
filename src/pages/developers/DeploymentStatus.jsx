import { useState, useMemo } from 'react';
import Button from '../../components/common/Button';
import Badge from '../../components/common/Badge';
import BuildLogs from './BuildLogs';
import './DeploymentStatus.css';

const STATUS_CONFIG = {
  pending: {
    label: 'Pending',
    variant: 'subtle',
    color: 'gray',
    description: 'Waiting to start build',
  },
  building: {
    label: 'Building',
    variant: 'subtle',
    color: 'amber',
    description: 'Build in progress',
  },
  success: {
    label: 'Success',
    variant: 'solid',
    color: 'green',
    description: 'Deployment ready',
  },
  failed: {
    label: 'Failed',
    variant: 'solid',
    color: 'red',
    description: 'Build failed',
  },
};

export default function DeploymentStatus({
  deployment,
  onRollback,
  onCancel,
}) {
  const [showLogs, setShowLogs] = useState(false);

  const status = deployment?.status || 'pending';
  const config = STATUS_CONFIG[status] || STATUS_CONFIG.pending;

  const currentBuildLogs = useMemo(() => deployment?.logs || '', [deployment?.logs]);

  const getProgressValue = () => {
    switch (status) {
      case 'pending': return 0;
      case 'building': return deployment?.progress || 50;
      case 'success': return 100;
      case 'failed': return 100;
      default: return 0;
    }
  };

  const getProgressColor = () => {
    switch (status) {
      case 'pending': return 'gray';
      case 'building': return 'warning';
      case 'success': return 'success';
      case 'failed': return 'danger';
      default: return 'primary';
    }
  };

  const handleRollback = () => {
    if (window.confirm('Are you sure you want to rollback to the previous version?')) {
      onRollback?.(deployment?.id);
    }
  };

  return (
    <div className="deployment-status">
      <div className="deployment-header">
        <div className="deployment-info">
          <div className="deployment-title">
            <h3>{deployment?.name || 'Deployment'}</h3>
            <Badge variant={config.variant} color={config.color} dot>
              {config.label}
            </Badge>
          </div>
          <p className="deployment-description">{config.description}</p>
        </div>

        <div className="deployment-meta">
          {deployment?.version && (
            <span className="deployment-version">v{deployment.version}</span>
          )}
          {deployment?.startedAt && (
            <span className="deployment-time">
              {new Date(deployment.startedAt).toLocaleString()}
            </span>
          )}
        </div>
      </div>

      {status === 'building' && (
        <div className="deployment-progress">
          <div className="progress-bar">
            <div
              className={`progress-fill progress-${getProgressColor()}`}
              style={{ width: `${getProgressValue()}%` }}
            />
          </div>
          <span className="progress-text">{getProgressValue()}%</span>
        </div>
      )}

      <div className="deployment-details">
        <div className="detail-item">
          <span className="detail-label">Repository</span>
          <span className="detail-value">{deployment?.repo || '—'}</span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Branch</span>
          <span className="detail-value">{deployment?.branch || '—'}</span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Commit</span>
          <span className="detail-value commit">
            {deployment?.commit?.slice(0, 7) || '—'}
            {deployment?.commit && (
              <button
                className="copy-commit"
                onClick={() => navigator.clipboard.writeText(deployment.commit)}
                title="Copy commit hash"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                </svg>
              </button>
            )}
          </span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Build Duration</span>
          <span className="detail-value">{deployment?.duration || '—'}</span>
        </div>
      </div>

      {deployment?.url && (
        <div className="deployment-url">
          <span className="url-label">Deployment URL</span>
          <a href={deployment.url} target="_blank" rel="noopener noreferrer" className="url-link">
            {deployment.url}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6M15 3h6v6M10 14L21 3" />
            </svg>
          </a>
        </div>
      )}

      {showLogs && (
        <div className="deployment-logs">
          <BuildLogs
            logs={currentBuildLogs}
            isBuilding={status === 'building'}
          />
        </div>
      )}

      <div className="deployment-actions">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowLogs(!showLogs)}
          leftIcon={
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
              <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
            </svg>
          }
        >
          {showLogs ? 'Hide Logs' : 'View Logs'}
        </Button>

        {status === 'building' && onCancel && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onCancel(deployment?.id)}
          >
            Cancel Build
          </Button>
        )}

        {status === 'success' && (
          <Button
            variant="secondary"
            size="sm"
            onClick={handleRollback}
            leftIcon={
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
              </svg>
            }
          >
            Rollback
          </Button>
        )}

        {status === 'failed' && onRollback && (
          <Button
            variant="primary"
            size="sm"
            onClick={() => onRollback(deployment?.id)}
            leftIcon={
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
              </svg>
            }
          >
            Retry Build
          </Button>
        )}
      </div>
    </div>
  );
}
