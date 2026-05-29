import { useState, useMemo } from 'react';
import Layout from '../components/Layout';
import LeaderboardRow from '../components/LeaderboardRow';
import { mockLeaderboard } from '../data/mockLeaderboard';
import './social.css';

const TIME_FILTERS = [
  { key: 'daily',    label: 'Daily'    },
  { key: 'weekly',   label: 'Weekly'   },
  { key: 'monthly',  label: 'Monthly'  },
  { key: 'all-time', label: 'All Time' },
];

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Raiders'   },
  { id: 2, title: 'Puzzle Dimension' },
  { id: 3, title: 'Speed Legends'    },
  { id: 4, title: 'Dungeon Depths'   },
  { id: 5, title: 'Strategy Command' },
  { id: 6, title: 'Retro Arcade'     },
];

const ITEMS_PER_PAGE = 10;

// Deterministic pseudo-random so same game+filter yields same numbers
function seededRand(a, b) {
  const x = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
  return x - Math.floor(x);
}

function buildLeaderboardData(gameId, timeFilter) {
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
  const [selectedGame, setSelectedGame] = useState(MOCK_GAMES[0].id);
  const [timeFilter, setTimeFilter]     = useState('all-time');
  const [currentPage, setCurrentPage]   = useState(1);

  const currentUser = { username: 'PlayerOne' };

  const leaderboardData = useMemo(
    () => buildLeaderboardData(selectedGame, timeFilter),
    [selectedGame, timeFilter]
  );

  const totalPages  = Math.ceil(leaderboardData.length / ITEMS_PER_PAGE);
  const startIndex  = (currentPage - 1) * ITEMS_PER_PAGE;
  const currentData = leaderboardData.slice(startIndex, startIndex + ITEMS_PER_PAGE);
  const userRank    = leaderboardData.findIndex(e => e.username === currentUser.username) + 1;

  const handleGameChange = (id) => { setSelectedGame(Number(id)); setCurrentPage(1); };
  const handleFilterChange = (key) => { setTimeFilter(key); setCurrentPage(1); };

  return (
    <Layout>
      <div className="leaderboard-page">
        <header className="page-header">
          <h1>Leaderboard</h1>
          <p>// COMPETE WORLDWIDE</p>
        </header>

        <div className="leaderboard-controls">
          <div className="game-select">
            <label htmlFor="game-selector">Game</label>
            <select
              id="game-selector"
              value={selectedGame}
              onChange={(e) => handleGameChange(e.target.value)}
            >
              {MOCK_GAMES.map(game => (
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

        {userRank > 0 && userRank > 3 && (
          <div className="your-rank-banner" role="status">
            Your rank: #{userRank}
          </div>
        )}

        <div className="leaderboard-container" role="table" aria-label="Leaderboard">
          <div className="leaderboard-header" role="row">
            <span className="col-rank" role="columnheader">Rank</span>
            <span className="col-player" role="columnheader">Player</span>
            <span className="col-score" role="columnheader">Score</span>
            <span className="col-change" role="columnheader">Change</span>
          </div>

          <div className="leaderboard-body">
            {currentData.map(entry => (
              <LeaderboardRow
                key={entry.rank}
                entry={entry}
                isCurrentUser={entry.username === currentUser.username}
                highlightTop3
              />
            ))}
          </div>
        </div>

        {totalPages > 1 && (
          <nav className="pagination" aria-label="Leaderboard pages">
            <button
              className="btn btn-secondary btn-sm"
              disabled={currentPage === 1}
              onClick={() => setCurrentPage(p => p - 1)}
              aria-label="Previous page"
            >
              Previous
            </button>
            <span className="page-info" aria-current="page">
              Page {currentPage} of {totalPages}
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
