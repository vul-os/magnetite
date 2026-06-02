import { useState, useMemo } from 'react';
import Button from '../../components/common/Button';
import Badge from '../../components/common/Badge';
import BuildLogs from './BuildLogs';
import BuildTimeline from './BuildTimeline';
import './DeploymentStatus.css';

const STATUS_CONFIG = {
  pending: {
    label: 'Pending',
    variant: 'subtle',
    color: 'gray',
    description: 'Waiting to start build',
  },
  queued: {
    label: 'Queued',
    variant: 'subtle',
    color: 'gray',
    description: 'Queued on self-hosted runner',
  },
  building: {
    label: 'Building',
    variant: 'subtle',
    color: 'amber',
    description: 'Build in progress',
  },
  built: {
    label: 'Live',
    variant: 'solid',
    color: 'green',
    description: 'Deployment live',
  },
  success: {
    label: 'Live',
    variant: 'solid',
    color: 'green',
    description: 'Deployment live',
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
  onViewLogs,
  onPromote,
}) {
  const [showLogs, setShowLogs]           = useState(false);
  const [showTimeline, setShowTimeline]   = useState(true);

  const status = deployment?.status || 'pending';
  const config = STATUS_CONFIG[status] || STATUS_CONFIG.pending;

  const currentBuildLogs = useMemo(() => deployment?.logs || '', [deployment?.logs]);

  const handleRollback = () => {
    if (window.confirm('Roll back to the previous version?')) {
      onRollback?.(deployment);
    }
  };

  const handleViewLogs = () => {
    if (onViewLogs) {
      onViewLogs(deployment);
    } else {
      setShowLogs(v => !v);
    }
  };

  return (
    <div className="deployment-status" data-status={status}>
      {/* Top accent bar by status */}
      <div className="ds-status-bar" aria-hidden="true" />

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
              {new Date(deployment.startedAt).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' })}
            </span>
          )}
          <button
            className="ds-toggle-btn"
            onClick={() => setShowTimeline(v => !v)}
            aria-expanded={showTimeline}
            aria-label={showTimeline ? 'Collapse pipeline' : 'Expand pipeline'}
            title={showTimeline ? 'Collapse pipeline' : 'Show pipeline'}
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 12 12"
              fill="none"
              aria-hidden="true"
              style={{ transform: showTimeline ? 'rotate(0deg)' : 'rotate(-90deg)', transition: 'transform 0.2s ease' }}
            >
              <path d="M2 4l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        </div>
      </div>

      {/* Build pipeline timeline */}
      {showTimeline && (
        <BuildTimeline
          status={status}
          startedAt={deployment?.startedAt}
          duration={deployment?.duration}
          commitSha={deployment?.commit}
        />
      )}

      {/* Detail grid — repo, branch, commit, duration */}
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

      {/* Inline logs (fallback when no onViewLogs handler) */}
      {showLogs && !onViewLogs && (
        <div className="deployment-logs">
          <BuildLogs
            logs={currentBuildLogs}
            isBuilding={status === 'building' || status === 'queued'}
          />
        </div>
      )}

      <div className="deployment-actions">
        <Button
          variant="ghost"
          size="sm"
          onClick={handleViewLogs}
          leftIcon={
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
              <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
            </svg>
          }
        >
          {onViewLogs ? 'View Logs' : (showLogs ? 'Hide Logs' : 'View Logs')}
        </Button>

        {(status === 'building' || status === 'queued') && onCancel && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onCancel(deployment?.id)}
          >
            Cancel
          </Button>
        )}

        {(status === 'built' || status === 'success') && onPromote && (
          <Button
            variant="primary"
            size="sm"
            onClick={() => onPromote(deployment)}
            leftIcon={
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 5v14M5 12l7-7 7 7" />
              </svg>
            }
          >
            Promote Live
          </Button>
        )}

        {(status === 'built' || status === 'success') && onRollback && (
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
            onClick={() => onRollback(deployment)}
            leftIcon={
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
              </svg>
            }
          >
            Retry
          </Button>
        )}
      </div>
    </div>
  );
}
