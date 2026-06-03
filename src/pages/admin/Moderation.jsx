/**
 * Moderation.jsx — Admin moderation queue page.
 *
 * Lists review_reports from GET /api/v1/admin/review-reports, filtered by
 * status (pending / resolved / all).  For each report an admin can:
 *   - Dismiss      → keeps the review, marks report resolved
 *   - Remove       → deletes the reported review
 *   - Warn user    → issues a warning to the reviewer
 *   - Remove & Ban → removes the review and bans the reviewer
 *
 * Admin endpoints:
 *   GET  /api/v1/admin/review-reports       (reviewReports)
 *   POST /api/v1/admin/review-reports/:id/dismiss (dismissReport)
 *   POST /api/v1/admin/users/:id/warn       (warnUser)
 *   POST /api/v1/admin/users/:id/ban        (banUser)
 */

import { useState, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import { api } from '../../api/client';
import './admin.css';
import './Moderation.css';

// ── Constants ────────────────────────────────────────────────────────────────

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';
const PAGE_SIZE = 20;

const STATUS_TABS = [
  { id: 'pending',  label: 'Pending' },
  { id: 'resolved', label: 'Resolved' },
  { id: 'all',      label: 'All' },
];

const REASON_META = {
  inappropriate:   { label: 'Inappropriate', color: '#ff5468', bg: 'rgba(255,84,104,0.1)' },
  spam:            { label: 'Spam',           color: '#f5a524', bg: 'rgba(245,165,36,0.1)' },
  false_information:{ label: 'False Info',   color: '#5b9dff', bg: 'rgba(91,157,255,0.1)' },
  harassment:      { label: 'Harassment',    color: '#ff5468', bg: 'rgba(255,84,104,0.1)' },
  other:           { label: 'Other',          color: '#6b6b78', bg: 'rgba(107,107,120,0.1)' },
};

const STATUS_META = {
  pending:   { label: 'Pending',   color: '#f5a524' },
  dismissed: { label: 'Dismissed', color: '#3ddc84' },
  actioned:  { label: 'Actioned',  color: '#38e1c8' },
  resolved:  { label: 'Resolved',  color: '#3ddc84' },
};

// ── Mock data ─────────────────────────────────────────────────────────────────

const MOCK_REPORTS = [
  {
    id: 'rpt_001',
    review_id: 'rev_001',
    review_content: 'This game is terrible. The developer stole my money and I want a refund immediately. Absolute scam.',
    reviewer_username: 'AngryGamer42',
    reviewer_id: 'usr_042',
    game_title: 'Cosmic Raiders',
    game_id: 'game_001',
    reason: 'inappropriate',
    auto_flag_reason: 'Sentiment score -0.94 · Toxicity 0.87 · Matched keyword: "scam"',
    reporter_username: 'NeonRacer99',
    reporter_id: 'usr_099',
    created_at: '2026-05-28T10:14:00Z',
    status: 'pending',
    review_rating: 1,
  },
  {
    id: 'rpt_002',
    review_id: 'rev_002',
    review_content: 'Check out my free game at h4xx.com/free — way better graphics! Click my profile for more.',
    reviewer_username: 'SpammerXYZ',
    reviewer_id: 'usr_777',
    game_title: 'Neon Drift',
    game_id: 'game_002',
    reason: 'spam',
    auto_flag_reason: 'External URL detected · Repeated pattern across 4 reviews',
    reporter_username: 'StarForge_Admin',
    reporter_id: 'usr_001',
    created_at: '2026-05-27T15:03:00Z',
    status: 'pending',
    review_rating: 5,
  },
  {
    id: 'rpt_003',
    review_id: 'rev_003',
    review_content: 'Average game, nothing special. Graphics are okay, gameplay is repetitive.',
    reviewer_username: 'CasualPlayer',
    reviewer_id: 'usr_234',
    game_title: 'Galaxy Conquest',
    game_id: 'game_003',
    reason: 'false_information',
    auto_flag_reason: null,
    reporter_username: 'GalacticDev',
    reporter_id: 'usr_055',
    created_at: '2026-05-26T09:22:00Z',
    status: 'dismissed',
    review_rating: 3,
  },
  {
    id: 'rpt_004',
    review_id: 'rev_004',
    review_content: 'This is absolute garbage. The dev team are complete idiots who have no idea what they are doing.',
    reviewer_username: 'ToxicGamer88',
    reviewer_id: 'usr_888',
    game_title: 'Stellar Forge',
    game_id: 'game_004',
    reason: 'harassment',
    auto_flag_reason: 'Toxicity score 0.96 · Matched harassment classifier',
    reporter_username: 'CommunityMod',
    reporter_id: 'usr_010',
    created_at: '2026-05-29T07:45:00Z',
    status: 'pending',
    review_rating: 1,
  },
];

// ── ReasonBadge ───────────────────────────────────────────────────────────────

function ReasonBadge({ reason }) {
  const meta = REASON_META[reason] ?? REASON_META.other;
  return (
    <span
      className="mod-reason-badge"
      style={{ color: meta.color, background: meta.bg, borderColor: `${meta.color}40` }}
    >
      {meta.label}
    </span>
  );
}

// ── StatusBadge ───────────────────────────────────────────────────────────────

function StatusBadge({ status }) {
  const meta = STATUS_META[status] ?? STATUS_META.pending;
  return (
    <span className="mod-status-badge" style={{ color: meta.color }}>
      {meta.label}
    </span>
  );
}

// ── StarRating ────────────────────────────────────────────────────────────────

function StarRating({ rating }) {
  if (!rating) return null;
  return (
    <span className="mod-star-rating" aria-label={`${rating} out of 5 stars`}>
      {Array.from({ length: 5 }, (_, i) => (
        <span key={i} className={`mod-star ${i < rating ? 'filled' : ''}`} aria-hidden="true">
          ★
        </span>
      ))}
    </span>
  );
}

// ── AutoFlagBanner ────────────────────────────────────────────────────────────

function AutoFlagBanner({ reason }) {
  if (!reason) return null;
  return (
    <div className="mod-auto-flag" role="note">
      <span className="mod-auto-flag-icon" aria-hidden="true">⚑</span>
      <span className="mod-auto-flag-label">Auto-flagged:</span>
      <span className="mod-auto-flag-text">{reason}</span>
    </div>
  );
}

// ── ReportCard ────────────────────────────────────────────────────────────────

function ReportCard({ report, actioning, actionMsg, onAction }) {
  const [expanded, setExpanded] = useState(false);
  const isPending = report.status === 'pending';

  return (
    <article
      className={`mod-card ${isPending ? 'mod-card--pending' : 'mod-card--resolved'}`}
      aria-label={`Report ${report.id}`}
    >
      {/* Card header — always visible */}
      <button
        className="mod-card-header"
        onClick={() => setExpanded(v => !v)}
        aria-expanded={expanded}
        aria-controls={`mod-body-${report.id}`}
      >
        <div className="mod-card-header-left">
          <ReasonBadge reason={report.reason} />
          <span className="mod-game-title">{report.game_title ?? 'Unknown game'}</span>
          <span className="mod-reviewer">
            Review by <strong>{report.reviewer_username ?? 'unknown'}</strong>
          </span>
          {report.review_rating && <StarRating rating={report.review_rating} />}
        </div>
        <div className="mod-card-header-right">
          <StatusBadge status={report.status} />
          <time
            className="mod-date"
            dateTime={report.created_at}
            title={new Date(report.created_at).toLocaleString()}
          >
            {new Date(report.created_at).toLocaleDateString('en-US', {
              month: 'short', day: 'numeric', year: 'numeric',
            })}
          </time>
          <span className="mod-expand-icon" aria-hidden="true">{expanded ? '▲' : '▼'}</span>
        </div>
      </button>

      {/* Card body — expanded */}
      {expanded && (
        <div className="mod-card-body" id={`mod-body-${report.id}`}>
          {/* Auto-flag banner */}
          <AutoFlagBanner reason={report.auto_flag_reason} />

          {/* Review content */}
          <div className="mod-review-block">
            <span className="kicker" style={{ fontSize: '0.65rem' }}>// REPORTED REVIEW</span>
            <blockquote className="mod-review-content">
              {report.review_content ?? 'No content available.'}
            </blockquote>
          </div>

          {/* Reporter info */}
          <div className="mod-reporter-row">
            <span className="mod-reporter-label">Reported by</span>
            <strong className="mod-reporter-name">{report.reporter_username ?? 'unknown'}</strong>
            <span className="mod-report-id">Report ID: <code>{report.id}</code></span>
          </div>

          {/* Action feedback */}
          {actionMsg && (
            <div
              role="status"
              aria-live="polite"
              className={`mod-action-msg ${actionMsg.isError ? 'mod-action-msg--error' : 'mod-action-msg--success'}`}
            >
              {actionMsg.text}
            </div>
          )}

          {/* Action buttons — only for pending */}
          {isPending && (
            <div className="mod-actions" role="group" aria-label="Moderation actions">
              <button
                className="mod-action-btn mod-action-btn--dismiss"
                onClick={() => onAction(report, 'dismiss')}
                disabled={!!actioning}
                aria-busy={actioning === 'dismiss'}
              >
                {actioning === 'dismiss' ? 'Working…' : 'Dismiss'}
              </button>
              <button
                className="mod-action-btn mod-action-btn--warn"
                onClick={() => onAction(report, 'warn')}
                disabled={!!actioning}
                aria-busy={actioning === 'warn'}
              >
                {actioning === 'warn' ? 'Working…' : 'Warn User'}
              </button>
              <button
                className="mod-action-btn mod-action-btn--remove"
                onClick={() => onAction(report, 'remove_review')}
                disabled={!!actioning}
                aria-busy={actioning === 'remove_review'}
              >
                {actioning === 'remove_review' ? 'Working…' : 'Remove Review'}
              </button>
              <button
                className="mod-action-btn mod-action-btn--ban"
                onClick={() => onAction(report, 'ban_user')}
                disabled={!!actioning}
                aria-busy={actioning === 'ban_user'}
              >
                {actioning === 'ban_user' ? 'Working…' : 'Remove & Ban'}
              </button>
            </div>
          )}
        </div>
      )}
    </article>
  );
}

// ── Empty state ───────────────────────────────────────────────────────────────

function EmptyState({ statusFilter }) {
  return (
    <div className="mod-empty">
      <div className="mod-empty-icon" aria-hidden="true">✓</div>
      <h3 className="mod-empty-title">
        {statusFilter === 'pending'
          ? 'Queue is clear'
          : `No ${statusFilter} reports`}
      </h3>
      <p className="mod-empty-desc">
        {statusFilter === 'pending'
          ? 'All reported reviews have been reviewed. Great work!'
          : `There are no ${statusFilter} reports to display.`}
      </p>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export default function Moderation() {
  const [reports, setReports]           = useState([]);
  const [loading, setLoading]           = useState(true);
  const [loadError, setLoadError]       = useState(null);
  const [statusFilter, setStatusFilter] = useState('pending');
  const [page, setPage]                 = useState(1);
  const [total, setTotal]               = useState(0);
  // Per-report action state: { [reportId]: { actioning: string|null, msg: {text,isError}|null } }
  const [reportStates, setReportStates] = useState({});

  // ── Fetch reports ─────────────────────────────────────────────────────────

  const loadReports = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      if (USE_MOCKS) {
        await new Promise(r => setTimeout(r, 280));
        const filtered = MOCK_REPORTS.filter(r =>
          statusFilter === 'all'      ? true :
          statusFilter === 'resolved' ? (r.status !== 'pending') :
                                        r.status === statusFilter
        );
        setReports(filtered);
        setTotal(filtered.length);
        return;
      }

      const params = {
        limit:  PAGE_SIZE,
        offset: (page - 1) * PAGE_SIZE,
        ...(statusFilter !== 'all' ? { status: statusFilter } : {}),
      };
      const data = await api.admin.reviewReports(params);
      const list = Array.isArray(data) ? data : (data?.reports ?? data?.items ?? []);
      setReports(list);
      setTotal(data?.total ?? list.length);
    } catch (err) {
      setLoadError(err.message || 'Failed to load moderation reports');
    } finally {
      setLoading(false);
    }
  }, [statusFilter, page]);

  useEffect(() => { loadReports(); }, [loadReports]);

  // ── Handle moderation actions ──────────────────────────────────────────────

  const setReportState = useCallback((reportId, updates) => {
    setReportStates(prev => ({
      ...prev,
      [reportId]: { ...(prev[reportId] ?? {}), ...updates },
    }));
  }, []);

  const handleAction = useCallback(async (report, action) => {
    const { id: reportId, reviewer_id: reviewerId } = report;
    setReportState(reportId, { actioning: action, msg: null });

    try {
      if (!USE_MOCKS) {
        if (action === 'warn') {
          // Warn user — separate endpoint
          await api.admin.warnUser(reviewerId, `Review flagged: ${report.reason}`);
          // Also dismiss the report
          await api.admin.dismissReport(reportId, { action: 'dismiss' });
        } else if (action === 'ban_user') {
          await api.admin.dismissReport(reportId, { action: 'ban_user' });
        } else {
          // dismiss | remove_review
          await api.admin.dismissReport(reportId, { action });
        }
      }

      // Optimistic update
      const newStatus = action === 'dismiss' ? 'dismissed' : 'actioned';
      setReports(prev => prev.map(r =>
        r.id === reportId ? { ...r, status: newStatus } : r
      ));

      const successMessages = {
        dismiss:       'Report dismissed — review kept.',
        warn:          'User warned and report dismissed.',
        remove_review: 'Review removed.',
        ban_user:      'Review removed and user banned.',
      };
      setReportState(reportId, {
        actioning: null,
        msg: { text: successMessages[action] ?? 'Done.', isError: false },
      });
      // Clear message after 5s
      setTimeout(() => setReportState(reportId, { msg: null }), 5000);
    } catch (err) {
      setReportState(reportId, {
        actioning: null,
        msg: { text: `Error: ${err.message || 'Action failed'}`, isError: true },
      });
    }
  }, [setReportState]);

  // ── Stats ─────────────────────────────────────────────────────────────────

  const pendingCount = reports.filter(r => r.status === 'pending').length;

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main" id="main-content">
          {/* Page header */}
          <header className="admin-header">
            <span className="kicker">// ADMIN · MODERATION</span>
            <h1>Moderation Queue</h1>
            <p className="admin-subtitle">
              Review flagged content, verify auto-flag reasons, and take moderation actions.
              {statusFilter === 'pending' && pendingCount > 0 && (
                <span className="mod-pending-count" aria-live="polite">
                  {pendingCount} pending
                </span>
              )}
            </p>
          </header>

          {/* Error banner */}
          {loadError && (
            <div role="alert" className="mod-load-error">
              {loadError}
              <button className="btn btn-sm" style={{ marginLeft: '1rem' }} onClick={loadReports}>
                Retry
              </button>
            </div>
          )}

          {/* Status filter tabs */}
          <div className="mod-tabs" role="tablist" aria-label="Report status filter">
            {STATUS_TABS.map(tab => (
              <button
                key={tab.id}
                role="tab"
                aria-selected={statusFilter === tab.id}
                className={`mod-tab ${statusFilter === tab.id ? 'mod-tab--active' : ''}`}
                onClick={() => { setStatusFilter(tab.id); setPage(1); }}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {/* Content */}
          {loading ? (
            <div className="admin-loading" aria-busy="true" aria-live="polite">
              <span className="spinner" aria-hidden="true" />
              <span>Loading reports…</span>
            </div>
          ) : reports.length === 0 ? (
            <EmptyState statusFilter={statusFilter} />
          ) : (
            <div className="mod-list" aria-label="Moderation reports">
              {reports.map(report => {
                const rs = reportStates[report.id] ?? {};
                return (
                  <ReportCard
                    key={report.id}
                    report={report}
                    actioning={rs.actioning ?? null}
                    actionMsg={rs.msg ?? null}
                    onAction={handleAction}
                  />
                );
              })}
            </div>
          )}

          {/* Pagination */}
          {!loading && total > PAGE_SIZE && (
            <div className="mod-pagination">
              <button
                className="btn btn-secondary btn-sm"
                disabled={page <= 1}
                onClick={() => setPage(p => p - 1)}
              >
                ← Previous
              </button>
              <span className="mod-page-info">
                Page {page} of {Math.ceil(total / PAGE_SIZE)}
              </span>
              <button
                className="btn btn-secondary btn-sm"
                disabled={page >= Math.ceil(total / PAGE_SIZE)}
                onClick={() => setPage(p => p + 1)}
              >
                Next →
              </button>
            </div>
          )}
        </main>
      </div>
    </Layout>
  );
}
