import { useState, useMemo, useEffect } from 'react';
import Layout from '../components/Layout';
import LeaderboardRow from '../components/LeaderboardRow';
import LeaderboardSkeleton from '../components/skeletons/LeaderboardSkeleton';
import EmptyState from '../components/empty/EmptyState';
import { mockLeaderboard } from '../data/mockLeaderboard';
import { api } from '../api/client';
import './social.css';

const TrophyIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d="M8 21h8" />
    <path d="M12 17v4" />
    <path d="M7 4H4a2 2 0 0 0-2 2v2c0 2.2 1.8 4 4 4h1" />
    <path d="M17 4h3a2 2 0 0 1 2 2v2c0 2.2-1.8 4-4 4h-1" />
    <path d="M7 4v8a5 5 0 0 0 10 0V4" />
  </svg>
);

const TIME_FILTERS = [
  { key: 'daily',    label: 'Daily'    },
  { key: 'weekly',   label: 'Weekly'   },
  { key: 'monthly',  label: 'Monthly'  },
  { key: 'all-time', label: 'All Time' },
];

const ITEMS_PER_PAGE = 10;

function seededRand(a, b) {
  const x = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
  return x - Math.floor(x);
}

function buildFallbackData(gameId, timeFilter) {
  const key = String(gameId);
  const baseData = mockLeaderboard[key] || mockLeaderboard['1'];
  const tf = TIME_FILTERS.findIndex(f => f.key === timeFilter);

  const enriched = baseData.map((entry, i) => ({
    ...entry,
    change: Math.round(seededRand(gameId + tf, i) * 10) - 3,
    avatar: `https://picsum.photos/seed/${entry.username}/100/100`,
  }));

  for (let i = 0; i < 20; i++) {
    enriched.push({
      rank:     baseData.length + i + 1,
      username: `Player${1000 + i}`,
      score:    Math.max(1000000 - i * 50000, 10000),
      change:   Math.round(seededRand(gameId + tf + 999, i) * 5) - 2,
      avatar:   `https://picsum.photos/seed/player${1000 + i}/100/100`,
    });
  }

  return enriched;
}

export default function Leaderboard() {
  const [games, setGames] = useState([
    { id: 1, title: 'Cosmic Raiders'   },
    { id: 2, title: 'Puzzle Dimension' },
    { id: 3, title: 'Speed Legends'    },
    { id: 4, title: 'Dungeon Depths'   },
    { id: 5, title: 'Strategy Command' },
    { id: 6, title: 'Retro Arcade'     },
  ]);
  const [selectedGame, setSelectedGame] = useState(1);
  const [timeFilter, setTimeFilter]     = useState('all-time');
  const [currentPage, setCurrentPage]   = useState(1);
  const [apiEntries, setApiEntries]     = useState(null);
  const [loadedGame, setLoadedGame]     = useState(null);
  /* loading = true until we've completed a fetch for the selectedGame */
  const loading = loadedGame !== selectedGame;

  useEffect(() => {
    let cancelled = false;
    api.games.list().then(data => {
      if (!cancelled && data) {
        const list = Array.isArray(data) ? data : (data?.games ?? null);
        if (list && list.length > 0) {
          setGames(list.map(g => ({ id: g.id, title: g.title })));
          setSelectedGame(list[0].id);
        }
      }
    }).catch(() => { /* use mock */ });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!selectedGame) return;
    let cancelled = false;

    api.games.leaderboard(selectedGame).then(data => {
      if (!cancelled) {
        const entries = data
          ? (Array.isArray(data) ? data : (data?.entries ?? null))
          : null;
        setApiEntries(entries && entries.length > 0 ? entries : null);
        setLoadedGame(selectedGame);
      }
    }).catch(() => {
      if (!cancelled) {
        setApiEntries(null);
        setLoadedGame(selectedGame);
      }
    });

    return () => { cancelled = true; };
  }, [selectedGame]);

  const currentUser = { username: 'PlayerOne' };

  const leaderboardData = useMemo(() => {
    if (apiEntries && apiEntries.length > 0) return apiEntries;
    return buildFallbackData(selectedGame, timeFilter);
  }, [apiEntries, selectedGame, timeFilter]);

  const totalPages  = Math.ceil(leaderboardData.length / ITEMS_PER_PAGE);
  const startIndex  = (currentPage - 1) * ITEMS_PER_PAGE;
  const currentData = leaderboardData.slice(startIndex, startIndex + ITEMS_PER_PAGE);
  const userRank    = leaderboardData.findIndex(e => e.username === currentUser.username) + 1;

  const handleGameChange = (id) => { setSelectedGame(Number(id)); setCurrentPage(1); setApiEntries(null); setLoadedGame(null); };
  const handleFilterChange = (key) => { setTimeFilter(key); setCurrentPage(1); };

  return (
    <Layout>
      <div className="leaderboard-page reveal">
        <header className="page-header reveal-1">
          <span className="kicker">// Compete Worldwide</span>
          <h1>Leaderboard</h1>
        </header>

        <div className="leaderboard-controls reveal-2">
          <div className="game-select">
            <label htmlFor="game-selector">Game</label>
            <select
              id="game-selector"
              value={selectedGame}
              onChange={(e) => handleGameChange(e.target.value)}
            >
              {games.map(game => (
                <option key={game.id} value={game.id}>{game.title}</option>
              ))}
            </select>
          </div>

          <div className="time-filters" role="group" aria-label="Time period">
            {TIME_FILTERS.map(filter => (
              <button
                key={filter.key}
                className={`filter-btn ${timeFilter === filter.key ? 'active' : ''}`}
                onClick={() => handleFilterChange(filter.key)}
                aria-pressed={timeFilter === filter.key}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>

        {!loading && userRank > 0 && userRank > 3 && (
          <div className="your-rank-banner reveal-3" role="status" aria-live="polite">
            Your rank: #{userRank}
          </div>
        )}

        <div
          className="leaderboard-container reveal-4"
          role="table"
          aria-label="Leaderboard"
          aria-busy={loading}
        >
          <div className="leaderboard-header" role="row">
            <span className="col-rank" role="columnheader">Rank</span>
            <span className="col-player" role="columnheader">Player</span>
            <span className="col-score" role="columnheader">Score</span>
            <span className="col-change" role="columnheader">Change</span>
          </div>

          <div className="leaderboard-body">
            {loading ? (
              Array.from({ length: ITEMS_PER_PAGE }).map((_, i) => (
                <LeaderboardSkeleton key={i} />
              ))
            ) : currentData.length === 0 ? (
              <div role="row">
                <EmptyState
                  icon={TrophyIcon}
                  title="No scores yet"
                  description="Be the first to make the leaderboard for this game."
                />
              </div>
            ) : (
              currentData.map(entry => (
                <LeaderboardRow
                  key={entry.rank}
                  entry={entry}
                  isCurrentUser={entry.username === currentUser.username}
                  highlightTop3
                />
              ))
            )}
          </div>
        </div>

        {!loading && totalPages > 1 && (
          <nav className="pagination reveal-5" aria-label="Leaderboard pages">
            <button
              className="btn btn-secondary btn-sm"
              disabled={currentPage === 1}
              onClick={() => setCurrentPage(p => p - 1)}
              aria-label="Previous page"
            >
              Previous
            </button>
            <span className="page-info" aria-current="page">
              {currentPage} / {totalPages}
            </span>
            <button
              className="btn btn-secondary btn-sm"
              disabled={currentPage === totalPages}
              onClick={() => setCurrentPage(p => p + 1)}
              aria-label="Next page"
            >
              Next
            </button>
          </nav>
        )}
      </div>
    </Layout>
  );
}
