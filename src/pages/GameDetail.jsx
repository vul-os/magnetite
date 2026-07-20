import { useState, useEffect, useRef } from 'react';
import { useParams } from 'react-router-dom';
import Layout from '../components/Layout';
import GameCard from '../components/GameCard';
import GameGallery from '../components/GameGallery';
import ReviewList from '../components/ReviewList';
import { api } from '../api/client';
import './GameDetail.css';

/*
 * GameDetail — "cold iron instrumentation".
 *
 * This page is a technical dossier on a verifiable artifact, not a storefront.
 * Its centre of gravity is PROVENANCE: who published the build, which version,
 * what hash, whether the simulation is replay-checked, where it runs.
 *
 * HARD RULE, enforced throughout: nothing numeric is ever invented. Every value
 * on this page either comes from the API or renders as an explicit
 * "not reported" / empty state. There is no placeholder rating, no placeholder
 * player count, no sample leaderboard. If it is not attested, we say so.
 */

const REQUIRED_TIER = {
  free: ['free'],
  basic: ['free', 'basic'],
  pro: ['free', 'basic', 'pro'],
  unlimited: ['free', 'basic', 'pro', 'unlimited'],
};

const TIER_NAMES = {
  free: 'Free',
  basic: 'Basic',
  pro: 'Pro',
  unlimited: 'Unlimited',
};

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

/*
 * Mock fixture, only used when VITE_USE_MOCKS === 'true'.
 *
 * It deliberately carries NO aggregate statistics — no rating, no leaderboard,
 * no reviews. A dev fixture that ships plausible-looking numbers is how invented
 * figures end up in screenshots and then in copy. The mock leaderboard/review
 * fixtures that used to live here were removed for exactly that reason; in mock
 * mode those surfaces render their real empty states.
 */
const MOCK_GAME = {
  id: 1,
  title: 'Cosmic Raiders',
  developer: 'StarForge Studios',
  developerId: 'starforge',
  requiredTier: 'basic',
  isFree: false,
  category: 'Action',
  description:
    'Embark on an interstellar adventure in Cosmic Raiders — a server-authoritative Rust game compiled to WebAssembly. Battle alien forces across 12 unique star systems, upgrade your spacecraft, and compete against players worldwide.',
  thumbnail: null,
  screenshots: [],
  video: null,
  github: null,
  release_date: '2026-03-15',
  players_min: 1,
  players_max: 4,
  system_requirements: {},
  achievements: [],
};

const TABS = ['Overview', 'Sessions', 'Leaderboard', 'Reviews'];

/* Content rating is publisher-self-declared — advisory, never verified here. */
const CONTENT_RATING_META = {
  everyone: { label: 'E', title: 'Everyone', description: 'Suitable for all ages' },
  teen:     { label: 'T', title: 'Teen',     description: 'Suitable for ages 13+' },
  mature:   { label: 'M', title: 'Mature',   description: 'Suitable for ages 17+' },
};

const AGE_GATE_REQUIRED_AGE = { everyone: 0, teen: 13, mature: 17 };

function ContentRatingBadge({ rating }) {
  const key = CONTENT_RATING_META[rating] ? rating : 'everyone';
  const meta = CONTENT_RATING_META[key];
  return (
    <span
      className={`gd-rating-badge gd-rating-${key}`}
      title={`${meta.title} — ${meta.description}`}
      aria-label={`Content rating: ${meta.title}`}
    >
      {meta.label}
    </span>
  );
}

/* Renders a value, or an explicit absence marker. Never a plausible stand-in. */
function DataValue({ value, className = '' }) {
  const missing =
    value === null || value === undefined || value === '' ||
    (typeof value === 'number' && Number.isNaN(value));
  if (missing) {
    return <span className="gd-unreported">— not reported</span>;
  }
  return <span className={`font-mono ${className}`}>{value}</span>;
}

function StarRating({ rating, size = 'md' }) {
  if (typeof rating !== 'number' || Number.isNaN(rating)) return null;
  const fullStars  = Math.floor(rating);
  const hasHalf    = rating % 1 >= 0.5;
  const emptyStars = Math.max(0, 5 - fullStars - (hasHalf ? 1 : 0));
  const px         = size === 'sm' ? 14 : size === 'lg' ? 22 : 16;

  return (
    <div className="star-rating" aria-label={`${rating.toFixed(1)} out of 5 stars`}>
      {[...Array(fullStars)].map((_, i) => (
        <svg key={`full-${i}`} className="star full" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      {hasHalf && (
        <svg className="star half" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      )}
      {[...Array(emptyStars)].map((_, i) => (
        <svg key={`empty-${i}`} className="star empty" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      <span className="rating-value font-mono" style={{ fontSize: px - 2 }}>{rating.toFixed(1)}</span>
    </div>
  );
}

function AchievementCard({ achievement }) {
  const has = typeof achievement.progress === 'number' && typeof achievement.total === 'number' && achievement.total > 0;
  const pct = has ? Math.round((achievement.progress / achievement.total) * 100) : null;
  return (
    <li className="achievement-card">
      <div className="achievement-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      </div>
      <div className="achievement-info">
        <h3>{achievement.name}</h3>
        {achievement.description && <p>{achievement.description}</p>}
        {has && (
          <div className="achievement-progress">
            <div
              className="progress-bar"
              role="progressbar"
              aria-valuenow={pct}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={`${achievement.name} progress`}
            >
              <div className="progress-fill" style={{ width: `${pct}%` }} />
            </div>
            <span className="progress-text font-mono">{achievement.progress}/{achievement.total}</span>
          </div>
        )}
      </div>
    </li>
  );
}

export default function GameDetail() {
  const { id } = useParams();

  const [game, setGame]                         = useState(USE_MOCKS ? MOCK_GAME : null);
  const [leaderboard, setLeaderboard]           = useState([]);
  const [reviews, setReviews]                   = useState([]);
  const [pageLoading, setPageLoading]           = useState(!USE_MOCKS);
  const [pageError, setPageError]               = useState(null);

  const [walletConnected, setWalletConnected]   = useState(false);
  const [inQueue, setInQueue]                   = useState(false);
  const [activeTab, setActiveTab]               = useState('Overview');
  const [wishlisted, setWishlisted]             = useState(false);
  const [showShareToast, setShowShareToast]     = useState(false);
  const [stickyBarVisible, setStickyBarVisible] = useState(false);
  const [userTier] = useState('basic');
  const [ageGatePassed, setAgeGatePassed] = useState(false);
  const heroRef = useRef(null);
  const tabRefs = useRef({});

  // Show the sticky launch bar once the masthead scrolls out of view.
  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => setStickyBarVisible(!entry.isIntersecting),
      { threshold: 0 }
    );
    const el = heroRef.current;
    if (el) observer.observe(el);
    return () => { if (el) observer.unobserve(el); };
  }, []);

  useEffect(() => {
    if (USE_MOCKS || !id) return;

    let cancelled = false;

    async function loadGame() {
      setPageLoading(true);
      setPageError(null);
      try {
        const [gameData, lbData, reviewData] = await Promise.allSettled([
          api.games.get(id),
          api.games.leaderboard(id),
          api.reviews.list(id),
        ]);

        if (cancelled) return;

        if (gameData.status === 'fulfilled') {
          const g = gameData.value?.data ?? gameData.value;
          setGame(g);
        } else {
          throw gameData.reason;
        }

        if (lbData.status === 'fulfilled') {
          const lb = lbData.value?.data ?? lbData.value;
          setLeaderboard(Array.isArray(lb) ? lb : (lb?.entries ?? []));
        }

        if (reviewData.status === 'fulfilled') {
          const rv = reviewData.value?.data ?? reviewData.value;
          setReviews(Array.isArray(rv) ? rv : (rv?.reviews ?? []));
        }
      } catch (err) {
        if (!cancelled) {
          setPageError(err.message || 'Failed to load game');
        }
      } finally {
        if (!cancelled) setPageLoading(false);
      }
    }

    loadGame();
    return () => { cancelled = true; };
  }, [id]);

  const userTierLevel     = Object.keys(REQUIRED_TIER).indexOf(userTier);
  const requiredTierLevel = Object.keys(REQUIRED_TIER).indexOf(game?.requiredTier ?? game?.required_tier ?? 'free');
  const hasAccess         = userTierLevel >= requiredTierLevel || game?.isFree || game?.is_free;
  const needsUpgrade      = !hasAccess && !(game?.isFree || game?.is_free);

  const handlePlayNow = () => {
    if (!walletConnected) { setWalletConnected(true); return; }
    if (needsUpgrade)     { window.location.href = '/pricing'; return; }
    setInQueue(true);
    setTimeout(() => setInQueue(false), 3000);
  };

  const handleShare = () => {
    navigator.clipboard.writeText(window.location.href);
    setShowShareToast(true);
    setTimeout(() => setShowShareToast(false), 2000);
  };

  const formatDate = (d) => {
    if (!d) return null;
    const parsed = new Date(d);
    if (Number.isNaN(parsed.getTime())) return null;
    return parsed.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });
  };

  // Roving-tabindex keyboard support for the tablist.
  const onTabKeyDown = (e) => {
    const i = TABS.indexOf(activeTab);
    let next = null;
    if (e.key === 'ArrowRight') next = TABS[(i + 1) % TABS.length];
    else if (e.key === 'ArrowLeft') next = TABS[(i - 1 + TABS.length) % TABS.length];
    else if (e.key === 'Home') next = TABS[0];
    else if (e.key === 'End') next = TABS[TABS.length - 1];
    if (next) {
      e.preventDefault();
      setActiveTab(next);
      tabRefs.current[next]?.focus();
    }
  };

  const playLabel = !walletConnected
    ? 'Connect & Launch'
    : inQueue
    ? 'Joining…'
    : needsUpgrade
    ? 'Upgrade to Launch'
    : 'Launch';

  // ─── Loading ──────────────────────────────────────────────────────────────
  if (pageLoading) {
    return (
      <Layout>
        <div className="game-detail">
          <div className="gd-shell">
            <div className="gd-skeleton" aria-busy="true" aria-live="polite">
              <span className="gd-vh">Loading game dossier</span>
              <span className="sk gd-sk-plate" />
              <span className="sk sk-title gd-sk-heading" />
              <span className="sk sk-text" style={{ width: '30%' }} />
              <span className="sk gd-sk-panel" />
              <span className="sk gd-sk-panel" />
            </div>
          </div>
        </div>
      </Layout>
    );
  }

  // ─── Error ────────────────────────────────────────────────────────────────
  if (pageError) {
    return (
      <Layout>
        <div className="game-detail">
          <div className="gd-shell">
            <div className="state state-error" role="alert">
              <h1 className="state-title gd-state-title">Could not load this game</h1>
              <p className="state-body font-mono break-key">{pageError}</p>
              <div className="state-actions">
                <a href="/marketplace" className="btn btn-primary">Back to catalogue</a>
              </div>
            </div>
          </div>
        </div>
      </Layout>
    );
  }

  // ─── Not found ────────────────────────────────────────────────────────────
  if (!game) {
    return (
      <Layout>
        <div className="game-detail">
          <div className="gd-shell">
            <div className="state state-empty">
              <h1 className="state-title gd-state-title">Game not found</h1>
              <p className="state-body">
                This node has no record of that content address. It may be listed on another tracker.
              </p>
              <div className="state-actions">
                <a href="/marketplace" className="btn btn-primary">Back to catalogue</a>
              </div>
            </div>
          </div>
        </div>
      </Layout>
    );
  }

  // ─── Age gate ─────────────────────────────────────────────────────────────
  const contentRating = game?.content_rating ?? game?.contentRating ?? 'everyone';
  const requiredAge   = AGE_GATE_REQUIRED_AGE[contentRating] ?? 0;

  if (requiredAge > 0 && !ageGatePassed) {
    return (
      <Layout>
        <div className="game-detail">
          <div className="gd-shell gd-gate-shell">
            <section className="panel gd-gate" aria-labelledby="gd-gate-title">
              <ContentRatingBadge rating={contentRating} />
              <h1 id="gd-gate-title" className="gd-gate-title">Age confirmation required</h1>
              <p className="gd-gate-body">
                <strong>{game.title}</strong> is self-declared{' '}
                <strong>{CONTENT_RATING_META[contentRating]?.title}</strong> by its publisher and
                states content suitable for ages {requiredAge} and over.
              </p>
              <p className="gd-gate-note m-sm">Publisher-declared · not verified by this node</p>
              <div className="gd-gate-actions">
                <button className="btn btn-primary" onClick={() => setAgeGatePassed(true)}>
                  I am {requiredAge} or older
                </button>
                <a href="/marketplace" className="btn btn-secondary">Go back</a>
              </div>
            </section>
          </div>
        </div>
      </Layout>
    );
  }

  // ─── Derived, never invented ──────────────────────────────────────────────
  const tierName      = TIER_NAMES[game.requiredTier ?? game.required_tier ?? 'free'];
  const isFree        = Boolean(game.isFree || game.is_free);
  const screenshots   = Array.isArray(game.screenshots) ? game.screenshots : [];
  const achievements  = Array.isArray(game.achievements) ? game.achievements : [];
  const sessions      = Array.isArray(game.sessions) ? game.sessions : [];
  const sysReq        = Object.entries(game.system_requirements ?? {});
  const similarGames  = Array.isArray(game.similar) ? game.similar : [];

  const publisherKey  = game.developer_id ?? game.developerId ?? null;
  const buildVersion  = game.live_version ?? game.liveVersion ?? game.version ?? null;
  const buildHash     = game.content_hash ?? game.contentHash ?? game.build_hash ?? game.buildHash ?? null;
  const artifactType  = game.artifact_type ?? game.artifactType ?? null;
  const sourceRepo    = game.github ?? game.github_repo ?? game.githubRepo ?? null;
  const listedAt      = formatDate(game.created_at ?? game.release_date ?? game.releaseDate);
  const tickRate      = game.tick_rate ?? game.tickRate ?? null;

  // Tri-state: true / false / unknown. Unknown is never rendered as "verified".
  const replayVerified = typeof game.replay_verified === 'boolean' ? game.replay_verified
                       : typeof game.replayVerified === 'boolean' ? game.replayVerified
                       : null;
  const signedBuild    = typeof game.signature_valid === 'boolean' ? game.signature_valid
                       : typeof game.signatureValid === 'boolean' ? game.signatureValid
                       : null;
  const hasArtifact    = typeof game.has_playable_artifact === 'boolean' ? game.has_playable_artifact
                       : typeof game.hasPlayableArtifact === 'boolean' ? game.hasPlayableArtifact
                       : null;

  const attestPill = (value, yes, no) =>
    value === true  ? <span className="st st-field">{yes}</span>
  : value === false ? <span className="st st-boundary">{no}</span>
  :                   <span className="st st-off">Not attested</span>;

  // Ratings are computed from the reviews actually returned by the API — this
  // page has no independent aggregate rating and must never display one.
  const ratedReviews = reviews.filter(r => typeof r.rating === 'number');
  const avgRating    = ratedReviews.length
    ? ratedReviews.reduce((s, r) => s + r.rating, 0) / ratedReviews.length
    : null;

  const panelId = (tab) => `gd-panel-${tab.toLowerCase()}`;
  const tabId   = (tab) => `gd-tab-${tab.toLowerCase()}`;

  const renderTabContent = () => {
    switch (activeTab) {
      // ── Overview ──────────────────────────────────────────────────────────
      case 'Overview':
        return (
          <div className="tab-overview">
            {screenshots.length > 0 && (
              <section aria-labelledby="gd-h-media">
                <h2 id="gd-h-media" className="gd-h">Media</h2>
                <GameGallery images={screenshots} title={game.title} />
              </section>
            )}

            <section aria-labelledby="gd-h-about">
              <h2 id="gd-h-about" className="gd-h">About this game</h2>
              {game.description ? (
                <div className="prose">
                  <p className="game-description">{game.description}</p>
                </div>
              ) : (
                <div className="state state-empty">
                  <p className="state-title">No description published</p>
                  <p className="state-body">The publisher has not supplied a description for this build.</p>
                </div>
              )}
            </section>

            {game.video && (
              <section aria-labelledby="gd-h-video">
                <h2 id="gd-h-video" className="gd-h">Gameplay video</h2>
                <p className="gd-boundary-note edge-boundary">
                  Embedded from a third party. Its contents are outside anything this node can verify.
                </p>
                <div className="video-container">
                  <iframe
                    src={game.video}
                    title={`${game.title} gameplay video`}
                    frameBorder="0"
                    allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                    allowFullScreen
                  />
                </div>
              </section>
            )}

            <section aria-labelledby="gd-h-ach">
              <div className="section-header-row">
                <h2 id="gd-h-ach" className="gd-h">Achievements</h2>
                {achievements.length > 0 && (
                  <span className="achievement-count font-mono">{achievements.length}</span>
                )}
              </div>
              {achievements.length > 0 ? (
                <ul className="achievements-grid">
                  {achievements.map((a, i) => (
                    <AchievementCard key={a.id ?? a.name ?? i} achievement={a} />
                  ))}
                </ul>
              ) : (
                <div className="state state-empty">
                  <p className="state-title">No achievements defined</p>
                  <p className="state-body">This build does not declare any achievements.</p>
                </div>
              )}
            </section>
          </div>
        );

      // ── Sessions ──────────────────────────────────────────────────────────
      case 'Sessions':
        return (
          <div className="tab-sessions">
            <section aria-labelledby="gd-h-sessions">
              <h2 id="gd-h-sessions" className="gd-h">Servers running this build</h2>
              <p className="gd-section-note">
                Anyone can host. Node keys and capacity are signed by the operator;
                operator name, region and player counts are self-declared and unattested.
              </p>
              {sessions.length > 0 ? (
                <div className="table-wrap">
                  <table className="data">
                    <caption className="gd-vh">Advertised sessions for {game.title}</caption>
                    <thead>
                      <tr>
                        <th scope="col">Node key</th>
                        <th scope="col">Operator</th>
                        <th scope="col">Region</th>
                        <th scope="col" className="num">Players</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sessions.map((s, i) => (
                        <tr key={s.id ?? s.node ?? i}>
                          <td className="key break-key">{s.node ?? s.node_key ?? '—'}</td>
                          <td>{s.operator ?? <span className="gd-unreported">— self-declared, absent</span>}</td>
                          <td>{s.region ?? <span className="gd-unreported">—</span>}</td>
                          <td className="num">
                            {typeof s.players === 'number'
                              ? `${s.players}${typeof s.max_players === 'number' ? ` / ${s.max_players}` : ''}`
                              : '—'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="state state-empty">
                  <p className="state-title">No sessions advertised to this node</p>
                  <p className="state-body">
                    Nobody is currently advertising a server for this build on the tracker you are
                    connected to. You can host one yourself, or connect to another tracker.
                  </p>
                </div>
              )}
            </section>
          </div>
        );

      // ── Leaderboard ───────────────────────────────────────────────────────
      case 'Leaderboard':
        return (
          <div className="tab-leaderboard">
            <section aria-labelledby="gd-h-lb">
              <div className="section-header-row">
                <h2 id="gd-h-lb" className="gd-h">Top players</h2>
                <a href="/leaderboard" className="view-all-link m-sm">Full leaderboard →</a>
              </div>
              {leaderboard.length > 0 ? (
                <div className="table-wrap">
                  <table className="data">
                    <caption className="gd-vh">Leaderboard for {game.title}</caption>
                    <thead>
                      <tr>
                        <th scope="col" className="num">Rank</th>
                        <th scope="col">Player</th>
                        <th scope="col" className="num">Score</th>
                      </tr>
                    </thead>
                    <tbody>
                      {leaderboard.map((entry, i) => (
                        <tr key={entry.user_id ?? entry.rank ?? i}>
                          <td className="num gd-rank">{typeof entry.rank === 'number' ? entry.rank : i + 1}</td>
                          <td className="lead">
                            <span className="gd-player">
                              {entry.avatar && (
                                <img src={entry.avatar} alt="" className="player-avatar" loading="lazy" />
                              )}
                              {entry.player ?? entry.username ?? '—'}
                            </span>
                          </td>
                          <td className="num">
                            {typeof entry.score === 'number' ? entry.score.toLocaleString() : '—'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="state state-empty">
                  <p className="state-title">Leaderboard unavailable</p>
                  <p className="state-body">
                    No scores have been reported for this build, or this node does not carry a
                    leaderboard for it.
                  </p>
                </div>
              )}
            </section>
          </div>
        );

      // ── Reviews ───────────────────────────────────────────────────────────
      case 'Reviews':
        return (
          <div className="tab-reviews">
            <section aria-labelledby="gd-h-reviews">
              <h2 id="gd-h-reviews" className="gd-h">Player reviews</h2>

              {ratedReviews.length > 0 ? (
                <div className="reviews-summary">
                  <div className="rating-overview">
                    <p className="big-rating font-mono">{avgRating.toFixed(1)}</p>
                    <StarRating rating={avgRating} size="lg" />
                    <p className="m-sm gd-rating-caption">
                      Mean of {ratedReviews.length} rated {ratedReviews.length === 1 ? 'review' : 'reviews'}
                    </p>
                  </div>
                  <div className="rating-breakdown">
                    {[5, 4, 3, 2, 1].map(stars => {
                      const count   = ratedReviews.filter(r => r.rating === stars).length;
                      const percent = (count / ratedReviews.length) * 100;
                      return (
                        <div key={stars} className="rating-row">
                          <span className="font-mono gd-rating-label">{stars}★</span>
                          <span className="rating-bar">
                            <span className="rating-fill" style={{ width: `${percent}%` }} />
                          </span>
                          <span className="rating-count font-mono">{count}</span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              ) : (
                <div className="state state-empty">
                  <p className="state-title">No ratings yet</p>
                  <p className="state-body">
                    This build has no rated reviews. No score is shown rather than an estimated one.
                  </p>
                </div>
              )}

              <ReviewList
                reviews={reviews.map((r, i) => ({ ...r, id: r.id ?? i }))}
                walletConnected={walletConnected}
                onCreateReview={() => {}}
                onHelpful={(reviewId) => {
                  setReviews(prev => prev.map((r, i) => {
                    const rId = r.id ?? i;
                    return rId === reviewId ? { ...r, helpful: (r.helpful ?? 0) + 1 } : r;
                  }));
                  api.reviews.helpful(id, reviewId).catch(() => null);
                }}
                onReport={(reviewId) => {
                  api.reviews.report(id, reviewId).catch(() => null);
                }}
              />
            </section>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <Layout>
      <div className="game-detail">
        {/* ── Sticky launch bar ─────────────────────────────────────────── */}
        <div className={`gd-sticky-bar ${stickyBarVisible ? 'visible' : ''}`} aria-hidden={!stickyBarVisible}>
          <div className="gd-sticky-inner">
            <div className="gd-sticky-info">
              <span className="gd-sticky-title">{game.title}</span>
              {game.developer && <span className="gd-sticky-dev font-mono">{game.developer}</span>}
            </div>
            <div className="gd-sticky-actions">
              <span className={`st ${hasAccess ? 'st-field' : 'st-spec'} gd-sticky-access`}>
                {hasAccess ? 'Included in plan' : `Requires ${tierName}`}
              </span>
              <button
                className="btn btn-primary"
                onClick={handlePlayNow}
                disabled={inQueue}
                tabIndex={stickyBarVisible ? 0 : -1}
              >
                {playLabel}
              </button>
            </div>
          </div>
        </div>

        <div className="gd-shell">
          {/* ── Masthead ───────────────────────────────────────────────── */}
          <header className="gd-masthead reveal reveal-1" ref={heroRef}>
            {game.thumbnail && (
              <div className="gd-plate">
                <img src={game.thumbnail} alt={`Cover art for ${game.title}`} className="gd-plate-img" loading="eager" />
              </div>
            )}

            <div className="gd-identity">
              <div className="gd-eyebrow">
                {game.category && <span className="m-sm gd-chip">{game.category}</span>}
                <span className="m-sm gd-chip">{isFree ? 'Free to play' : `${tierName} tier`}</span>
                {(game.content_rating || game.contentRating) && (
                  <ContentRatingBadge rating={game.content_rating ?? game.contentRating} />
                )}
              </div>

              <h1 className="gd-title">{game.title}</h1>

              <p className="gd-publisher">
                Published by{' '}
                {publisherKey ? (
                  <a href={`/developer/${publisherKey}`}>{game.developer ?? 'Unknown publisher'}</a>
                ) : (
                  <span>{game.developer ?? 'Unknown publisher'}</span>
                )}
              </p>

              <ul className="gd-attest">
                <li>
                  <span className="m-sm gd-attest-label">Signed build</span>
                  {attestPill(signedBuild, 'Signature valid', 'Unsigned')}
                </li>
                <li>
                  <span className="m-sm gd-attest-label">Replay verification</span>
                  {attestPill(replayVerified, 'Replay-checked', 'Not replay-checked')}
                </li>
                <li>
                  <span className="m-sm gd-attest-label">Playable artifact</span>
                  {hasArtifact === true
                    ? <span className="st st-live">Available</span>
                    : hasArtifact === false
                    ? <span className="st st-off">None published</span>
                    : <span className="st st-off">Not reported</span>}
                </li>
              </ul>

              <div className="gd-actions">
                <button
                  className="btn btn-primary btn-lg"
                  onClick={handlePlayNow}
                  disabled={inQueue}
                  aria-label={
                    !walletConnected ? 'Connect wallet and launch game' :
                    inQueue ? 'Joining game queue' :
                    needsUpgrade ? `Upgrade to ${tierName} to launch` :
                    'Launch game'
                  }
                >
                  {playLabel}
                </button>
                <button
                  className={`btn btn-secondary gd-icon-btn ${wishlisted ? 'active' : ''}`}
                  onClick={() => setWishlisted(w => !w)}
                  aria-label={wishlisted ? 'Remove from wishlist' : 'Add to wishlist'}
                  aria-pressed={wishlisted}
                >
                  <svg viewBox="0 0 24 24" fill={wishlisted ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
                  </svg>
                </button>
                <span className={`st ${isFree ? 'st-live' : hasAccess ? 'st-field' : 'st-spec'} gd-access`}>
                  {isFree ? 'Free to play' : hasAccess ? 'Included in your plan' : `Requires ${tierName}`}
                </span>
              </div>

              {needsUpgrade && (
                <div className="gd-upgrade edge-spec" role="alert">
                  <p>Your current plan does not include this build.</p>
                  <a href="/pricing" className="btn btn-secondary btn-sm">View plans</a>
                </div>
              )}
            </div>
          </header>

          {/* ── Provenance dossier ─────────────────────────────────────── */}
          <section className="panel gd-provenance reveal reveal-2" aria-labelledby="gd-h-prov">
            <div className="gd-prov-head">
              <span className="kicker">Provenance</span>
              <h2 id="gd-h-prov" className="gd-h">What this node can and cannot prove</h2>
            </div>

            <dl className="gd-prov-list">
              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Publisher key</dt>
                <dd><DataValue value={publisherKey} className="break-key" /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Build version</dt>
                <dd><DataValue value={buildVersion} /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Content hash</dt>
                <dd><DataValue value={buildHash} className="break-key" /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Artifact runtime</dt>
                <dd><DataValue value={artifactType} /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Simulation tick rate</dt>
                <dd><DataValue value={typeof tickRate === 'number' ? `${tickRate} Hz` : tickRate} /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Listed</dt>
                <dd><DataValue value={listedAt} /></dd>
              </div>

              <div className="gd-prov-row edge-field">
                <dt className="m-sm">Catalogue status</dt>
                <dd><DataValue value={game.status} /></dd>
              </div>

              <div className="gd-prov-row edge-spec">
                <dt className="m-sm">Content rating</dt>
                <dd>
                  <span className="font-mono">{CONTENT_RATING_META[contentRating]?.title ?? 'Unrated'}</span>
                  <span className="gd-prov-note">Self-declared by the publisher — advisory only.</span>
                </dd>
              </div>

              <div className="gd-prov-row edge-boundary">
                <dt className="m-sm">Source repository</dt>
                <dd>
                  {sourceRepo ? (
                    <>
                      <a
                        href={sourceRepo}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="font-mono break-key gd-boundary-link"
                      >
                        {sourceRepo} ↗
                      </a>
                      <span className="gd-prov-note">
                        Leaves Magnetite. Nothing beyond this link is attested by this node.
                      </span>
                    </>
                  ) : (
                    <DataValue value={null} />
                  )}
                </dd>
              </div>
            </dl>
          </section>

          {/* ── Spec strip ─────────────────────────────────────────────── */}
          {(game.players_min != null || game.players_max != null || listedAt) && (
            <section className="gd-spec-strip reveal reveal-3" aria-label="Build specification">
              {(game.players_min != null || game.players_max != null) && (
                <div className="gd-spec-item">
                  <span className="m-sm">Players</span>
                  <DataValue
                    value={
                      game.players_min != null && game.players_max != null
                        ? `${game.players_min}–${game.players_max}`
                        : (game.players_min ?? game.players_max)
                    }
                  />
                </div>
              )}
              {listedAt && (
                <div className="gd-spec-item">
                  <span className="m-sm">Listed</span>
                  <DataValue value={listedAt} />
                </div>
              )}
              <div className="gd-spec-item">
                <span className="m-sm">Runtime</span>
                <DataValue value={artifactType} />
              </div>
            </section>
          )}

          {/* ── Main grid ──────────────────────────────────────────────── */}
          <div className="game-content reveal reveal-4">
            <main className="game-main">
              <div className="tabs-section panel">
                <div className="tabs-nav" role="tablist" aria-label="Game detail sections" onKeyDown={onTabKeyDown}>
                  {TABS.map(tab => (
                    <button
                      key={tab}
                      id={tabId(tab)}
                      type="button"
                      role="tab"
                      ref={(el) => { tabRefs.current[tab] = el; }}
                      aria-selected={activeTab === tab}
                      aria-controls={panelId(tab)}
                      tabIndex={activeTab === tab ? 0 : -1}
                      className={`tab-btn ${activeTab === tab ? 'active' : ''}`}
                      onClick={() => setActiveTab(tab)}
                    >
                      {tab}
                    </button>
                  ))}
                </div>
                <div
                  className="tab-content"
                  role="tabpanel"
                  id={panelId(activeTab)}
                  aria-labelledby={tabId(activeTab)}
                  tabIndex={0}
                >
                  {renderTabContent()}
                </div>
              </div>
            </main>

            <aside className="game-sidebar" aria-label="Build details">
              <section className="panel sidebar-card">
                <h2 className="gd-h gd-sidebar-h">Publisher</h2>
                <div className="developer-info">
                  <span className="developer-monogram font-mono" aria-hidden="true">
                    {(game.developer ?? '?').trim().charAt(0).toUpperCase()}
                  </span>
                  <div className="developer-details">
                    <span className="developer-name">{game.developer ?? 'Unknown publisher'}</span>
                    {publisherKey && (
                      <>
                        <span className="font-mono break-key developer-key">{publisherKey}</span>
                        <a href={`/developer/${publisherKey}`} className="developer-link m-sm">
                          View profile →
                        </a>
                      </>
                    )}
                  </div>
                </div>
              </section>

              <section className="panel sidebar-card">
                <h2 className="gd-h gd-sidebar-h">System requirements</h2>
                {sysReq.length > 0 ? (
                  <dl className="system-req-list">
                    {sysReq.map(([key, value]) => (
                      <div key={key} className="system-req-item">
                        <dt className="m-sm req-label">{key}</dt>
                        <dd className="font-mono req-value">{String(value)}</dd>
                      </div>
                    ))}
                  </dl>
                ) : (
                  <p className="gd-unreported">— not reported</p>
                )}
              </section>

              {similarGames.length > 0 && (
                <section className="panel sidebar-card">
                  <h2 className="gd-h gd-sidebar-h">Similar games</h2>
                  <div className="similar-games-list">
                    {similarGames.map(g => (
                      <GameCard key={g.id} game={g} showPlayButton={false} />
                    ))}
                  </div>
                </section>
              )}

              <section className="panel sidebar-card">
                <h2 className="gd-h gd-sidebar-h">Actions</h2>
                <div className="action-links">
                  <button type="button" className="action-link-btn" onClick={handleShare}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                      <circle cx="18" cy="5" r="3" />
                      <circle cx="6" cy="12" r="3" />
                      <circle cx="18" cy="19" r="3" />
                      <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
                      <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
                    </svg>
                    Copy link to this build
                  </button>
                  <button type="button" className="action-link-btn danger">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" />
                      <line x1="12" y1="8" x2="12" y2="12" />
                      <line x1="12" y1="16" x2="12.01" y2="16" />
                    </svg>
                    Report an issue
                  </button>
                </div>
              </section>
            </aside>
          </div>
        </div>

        {showShareToast && (
          <div className="toast-notification font-mono" role="status" aria-live="polite">
            Link copied to clipboard
          </div>
        )}
      </div>
    </Layout>
  );
}
