/**
 * BuildTimeline — shows queued → building → built/failed as a sequential
 * timeline with honest messaging about the self-hosted CI runner.
 *
 * Props:
 *   status  {string}  — 'queued' | 'building' | 'built' | 'failed'
 *   startedAt {string} — ISO timestamp
 *   duration  {string} — human-readable duration string (e.g. "4m 32s")
 *   commitSha {string} — short commit hash
 */
import './BuildTimeline.css';

const STAGES = [
  {
    id: 'queued',
    label: 'Queued',
    desc: 'Build job enqueued on self-hosted runner',
    icon: (
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.5" />
        <path d="M8 4.5v3.5l2 1.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: 'building',
    label: 'Building',
    desc: 'cargo build --target wasm32-wasip1 --release',
    icon: (
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M2 11l4-4 3 3 5-6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: 'uploading',
    label: 'Uploading',
    desc: 'Artifact uploaded to distribution CDN',
    icon: (
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M8 10V4M5 7l3-3 3 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        <path d="M3 12h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: 'built',
    label: 'Live',
    desc: 'Deployed — play manifest active',
    icon: (
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M3 8l3.5 3.5L13 4" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
];

// Map API status → which STAGES are done/active/pending
function resolveStageState(status, stageId) {
  const order = ['queued', 'building', 'uploading', 'built'];
  const failedAt = { queued: 0, building: 1, uploading: 2 };

  if (status === 'failed') {
    const failIdx = failedAt[status] ?? 1; // default to failed at build
    const stageIdx = order.indexOf(stageId);
    if (stageIdx < failIdx) return 'done';
    if (stageIdx === failIdx) return 'failed';
    return 'pending';
  }

  // 'built' maps to success for the final stage
  const effectiveStatus = status === 'success' ? 'built' : status;
  const currentIdx = order.indexOf(effectiveStatus);
  const stageIdx = order.indexOf(stageId);

  if (stageIdx < currentIdx) return 'done';
  if (stageIdx === currentIdx) return 'active';
  return 'pending';
}

export default function BuildTimeline({ status = 'queued', startedAt, duration, commitSha }) {
  const isFailed = status === 'failed';
  const isBuilt  = status === 'built' || status === 'success';
  const isActive = status === 'building' || status === 'queued' || status === 'uploading';

  return (
    <div className="build-timeline" aria-label="Build pipeline timeline">
      {/* Honest CI runner note */}
      <div className="bt-runner-note" role="note">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeWidth="1.25" />
          <path d="M8 5v3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <circle cx="8" cy="11" r="0.75" fill="currentColor" />
        </svg>
        Self-hosted CI runner (Bucket D) — builds run on your infrastructure; cloud auto-scale in roadmap.
      </div>

      {/* Timeline stages */}
      <div className="bt-stages" role="list">
        {STAGES.map((stage, i) => {
          // For 'failed' we don't know which stage it failed at without more info,
          // so conservatively mark 'queued' as done and 'building' as failed.
          let stageState;
          if (status === 'failed') {
            if (i === 0) stageState = 'done';
            else if (i === 1) stageState = 'failed';
            else stageState = 'pending';
          } else {
            stageState = resolveStageState(status, stage.id);
          }

          const isDone    = stageState === 'done';
          const isActive2 = stageState === 'active';
          const isFailed2 = stageState === 'failed';

          return (
            <div
              key={stage.id}
              className={[
                'bt-stage',
                isDone    ? 'bt-done'    : '',
                isActive2 ? 'bt-active'  : '',
                isFailed2 ? 'bt-failed'  : '',
                i === STAGES.length - 1 ? 'bt-last' : '',
              ].join(' ')}
              role="listitem"
              aria-label={`${stage.label}: ${stageState}`}
            >
              {/* Connector line (not on last) */}
              {i < STAGES.length - 1 && (
                <div className={`bt-connector ${isDone ? 'bt-connector-done' : ''}`} aria-hidden="true" />
              )}

              {/* Node */}
              <div className="bt-node" aria-hidden="true">
                {isActive2 && <span className="bt-pulse" />}
                <span className="bt-node-inner">
                  {isFailed2 ? (
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                      <path d="M2 2l6 6M8 2L2 8" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
                    </svg>
                  ) : isDone ? (
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                      <path d="M2 5.5l2 2 4-4" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  ) : stage.icon}
                </span>
              </div>

              {/* Label */}
              <div className="bt-label-block">
                <span className="bt-stage-label">{stage.label}</span>
                <span className="bt-stage-desc">{stage.desc}</span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Footer meta */}
      {(startedAt || duration || commitSha) && (
        <div className="bt-meta">
          {startedAt && (
            <span className="bt-meta-item">
              <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.5" />
                <path d="M8 5v3l2 1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
              {new Date(startedAt).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' })}
            </span>
          )}
          {duration && (
            <span className="bt-meta-item">
              <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="M3 8h10M8 3v10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
              {duration}
            </span>
          )}
          {commitSha && (
            <span className="bt-meta-item bt-commit">
              <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="8" cy="8" r="2.5" stroke="currentColor" strokeWidth="1.5" />
                <path d="M1 8h4M11 8h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
              {commitSha.slice(0, 8)}
            </span>
          )}
        </div>
      )}

      {/* Status messages */}
      {isFailed && (
        <div className="bt-status-msg bt-status-failed" role="alert">
          Build failed — check logs above. Common causes: missing wasm32-wasip1 target, dependency errors, or out-of-memory on the runner.
        </div>
      )}
      {isBuilt && (
        <div className="bt-status-msg bt-status-built">
          Build live — the play manifest is active. Players can connect now.
        </div>
      )}
      {isActive && (
        <div className="bt-status-msg bt-status-active" aria-live="polite">
          Build in progress — this page auto-refreshes every 5 s via build API polling.
        </div>
      )}
    </div>
  );
}
