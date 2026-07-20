import { useState, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import { api } from '../../api/client';
import './admin.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

const MOCK_REPORTS = [
  {
    id: 'rpt_001',
    review_id: 'rev_001',
    review_content: 'This game is terrible. The developer stole my money.',
    reviewer_username: 'AngryGamer42',
    game_title: 'Cosmic Raiders',
    reason: 'inappropriate',
    reporter_username: 'admin_flag',
    created_at: '2026-05-28T10:14:00Z',
    status: 'pending',
  },
  {
    id: 'rpt_002',
    review_id: 'rev_002',
    review_content: 'Check out my free game at h4xx.com/free',
    reviewer_username: 'SpammerXYZ',
    game_title: 'Neon Drift',
    reason: 'spam',
    reporter_username: 'NeonRacer99',
    created_at: '2026-05-27T15:03:00Z',
    status: 'pending',
  },
  {
    id: 'rpt_003',
    review_id: 'rev_003',
    review_content: 'Average game, nothing special.',
    reviewer_username: 'CasualPlayer',
    game_title: 'Galaxy Conquest',
    reason: 'false_information',
    reporter_username: 'StarForge_Admin',
    created_at: '2026-05-26T09:22:00Z',
    status: 'dismissed',
  },
];

const REASON_LABELS = {
  inappropriate: 'Inappropriate Content',
  spam: 'Spam / Advertising',
  false_information: 'False Information',
  harassment: 'Harassment',
  other: 'Other',
};

const STATUS_FILTERS = ['all', 'pending', 'dismissed', 'actioned'];

function ReportRow({ report, onDismiss, onRemove, onBan, actioning }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={`admin-report-row ${report.status !== 'pending' ? 'resolved' : ''}`}>
      <div className="report-header" onClick={() => setExpanded(v => !v)} style={{ cursor: 'pointer' }}>
        <div className="report-meta">
          <span className="report-reason-badge" data-reason={report.reason}>
            {REASON_LABELS[report.reason] ?? report.reason}
          </span>
          <span className="report-game">{report.game_title ?? 'Unknown Game'}</span>
          <span className="report-reviewer">
            by <strong>{report.reviewer_username ?? 'Unknown'}</strong>
          </span>
          <span className="report-status" data-status={report.status}>{report.status}</span>
        </div>
        <div className="report-date">
          {new Date(report.created_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}
        </div>
      </div>

      {expanded && (
        <div className="report-body">
          <div className="report-review-content">
            <span className="kicker" style={{ fontSize: '0.65rem' }}>// REVIEW CONTENT</span>
            <blockquote style={{ margin: '0.5rem 0', padding: '0.75rem', background: 'var(--color-bg-elevated)', borderLeft: '3px solid var(--color-border-strong)', borderRadius: '0 var(--radius-sm) var(--radius-sm) 0', fontSize: '0.875rem', color: 'var(--color-text-secondary)', fontFamily: 'var(--font-sans)', whiteSpace: 'pre-wrap', overflowWrap: 'break-word' }}>
              {report.review_content ?? 'No content available.'}
            </blockquote>
          </div>
          <div style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', marginBottom: '0.75rem' }}>
            Reported by <strong>{report.reporter_username ?? 'unknown'}</strong>
            &nbsp;&middot;&nbsp;Report ID: <code>{report.id}</code>
          </div>

          {report.status === 'pending' && (
            <div className="report-actions">
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => onDismiss(report.id)}
                disabled={actioning === report.id}
                aria-label="Dismiss this report — keep the review"
              >
                {actioning === report.id ? 'Working…' : 'Dismiss'}
              </button>
              <button
                className="btn btn-danger btn-sm"
                onClick={() => onRemove(report.id)}
                disabled={actioning === report.id}
                aria-label="Remove the review"
              >
                Remove Review
              </button>
              <button
                className="btn btn-sm"
                style={{ border: '1px solid var(--color-error)', color: 'var(--color-error)', background: 'none' }}
                onClick={() => onBan(report.id, report.reviewer_username)}
                disabled={actioning === report.id}
                aria-label="Remove review and ban the reviewer"
              >
                Remove &amp; Ban User
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function ReviewModeration() {
  const [reports, setReports]       = useState([]);
  const [loading, setLoading]       = useState(true);
  const [loadError, setLoadError]   = useState(null);
  const [statusFilter, setStatusFilter] = useState('pending');
  const [actioning, setActioning]   = useState(null);
  const [actionMsg, setActionMsg]   = useState(null);
  const [page, setPage]             = useState(1);
  const [total, setTotal]           = useState(0);
  const PAGE_SIZE = 20;

  const loadReports = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      if (USE_MOCKS) {
        await new Promise(r => setTimeout(r, 300)); // simulate latency
        const filtered = MOCK_REPORTS.filter(r =>
          statusFilter === 'all' ? true : r.status === statusFilter
        );
        setReports(filtered);
        setTotal(filtered.length);
        return;
      }
      const data = await api.admin.reviewReports({
        limit: PAGE_SIZE,
        offset: (page - 1) * PAGE_SIZE,
        ...(statusFilter !== 'all' ? { status: statusFilter } : {}),
      });
      const list = Array.isArray(data) ? data : (data?.reports ?? data?.items ?? []);
      setReports(list);
      setTotal(data?.total ?? list.length);
    } catch (err) {
      setLoadError(err.message || 'Failed to load review reports');
    } finally {
      setLoading(false);
    }
  }, [statusFilter, page]);

  // Load review-moderation reports from the admin API (external system).
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { loadReports(); }, [loadReports]);

  const applyAction = useCallback(async (reportId, action) => {
    setActioning(reportId);
    setActionMsg(null);
    try {
      if (!USE_MOCKS) {
        await api.admin.actOnReport(reportId, { action });
      }
      setReports(prev =>
        prev.map(r => r.id === reportId ? { ...r, status: action === 'dismiss' ? 'dismissed' : 'actioned' } : r)
      );
      const msgs = {
        dismiss: 'Report dismissed — review kept.',
        remove_review: 'Review removed.',
        ban_user: 'Review removed and user banned.',
      };
      setActionMsg(msgs[action] ?? 'Done.');
      setTimeout(() => setActionMsg(null), 4000);
    } catch (err) {
      setActionMsg(`Error: ${err.message}`);
    } finally {
      setActioning(null);
    }
  }, []);

  const pendingCount = reports.filter(r => r.status === 'pending').length;

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main" id="main-content">
          <div className="admin-header">
            <span className="kicker">// ADMIN</span>
            <h1>Review Moderation</h1>
            <p className="admin-subtitle">
              Review reports from users — dismiss false reports or remove offending content.
              {pendingCount > 0 && statusFilter === 'pending' && (
                <span style={{ marginLeft: '0.5rem', color: 'var(--color-warning)', fontFamily: 'var(--font-mono)', fontSize: '0.8rem' }}>
                  {pendingCount} pending
                </span>
              )}
            </p>
          </div>

          {actionMsg && (
            <div role="status" style={{ marginBottom: '1rem', padding: '0.75rem 1rem', background: 'rgba(61,220,132,0.1)', border: '1px solid var(--color-success)', borderRadius: 'var(--radius)', color: 'var(--color-success)', fontSize: '0.875rem', fontFamily: 'var(--font-mono)' }}>
              {actionMsg}
            </div>
          )}

          {loadError && (
            <div role="alert" style={{ marginBottom: '1rem', padding: '0.75rem 1rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
              {loadError}
              <button className="btn btn-sm" style={{ marginLeft: '1rem' }} onClick={loadReports}>Retry</button>
            </div>
          )}

          <div style={{ display: 'flex', gap: '0.5rem', marginBottom: '1.25rem', flexWrap: 'wrap', alignItems: 'center' }}>
            {STATUS_FILTERS.map(s => (
              <button
                key={s}
                className={`tab ${statusFilter === s ? 'active' : ''}`}
                role="tab"
                aria-selected={statusFilter === s}
                onClick={() => { setStatusFilter(s); setPage(1); }}
                style={{ textTransform: 'capitalize' }}
              >
                {s === 'all' ? 'All' : s}
              </button>
            ))}
          </div>

          {loading ? (
            <div className="admin-loading" aria-busy="true" aria-live="polite">
              <span className="spinner" aria-hidden="true" />
              <span>Loading reports…</span>
            </div>
          ) : reports.length === 0 ? (
            <div className="admin-empty-state">
              <span style={{ fontSize: '2rem', display: 'block', marginBottom: '0.5rem' }}>✓</span>
              <p>No {statusFilter !== 'all' ? statusFilter : ''} reports.</p>
              {statusFilter === 'pending' && (
                <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)' }}>
                  All reported reviews have been reviewed — the moderation queue is clear.
                </p>
              )}
            </div>
          ) : (
            <div className="admin-reports-list">
              {reports.map(report => (
                <ReportRow
                  key={report.id}
                  report={report}
                  actioning={actioning}
                  onDismiss={(id) => applyAction(id, 'dismiss')}
                  onRemove={(id) => applyAction(id, 'remove_review')}
                  onBan={(id) => applyAction(id, 'ban_user')}
                />
              ))}
            </div>
          )}

          {total > PAGE_SIZE && (
            <div style={{ marginTop: '1.5rem' }}>
              <Pagination
                currentPage={page}
                total={total}
                perPage={PAGE_SIZE}
                onPageChange={setPage}
              />
            </div>
          )}
        </main>
      </div>
    </Layout>
  );
}
