import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import LeaderboardRow from '../components/LeaderboardRow';
import Pagination from '../components/Pagination';
import { usePagination } from '../hooks/usePagination';
import { mockLeaderboard } from '../data/mockLeaderboard';

const TIME_FILTERS = [
  { key: 'daily', label: 'Daily' },
  { key: 'weekly', label: 'Weekly' },
  { key: 'monthly', label: 'Monthly' },
  { key: 'all-time', label: 'All Time' },
];

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Raiders' },
  { id: 2, title: 'Puzzle Dimension' },
  { id: 3, title: 'Speed Legends' },
  { id: 4, title: 'Dungeon Depths' },
  { id: 5, title: 'Strategy Command' },
  { id: 6, title: 'Retro Arcade' },
];

const ITEMS_PER_PAGE = 10;

export default function Leaderboard() {
  const [selectedGame, setSelectedGame] = useState(MOCK_GAMES[0].id);
  const [timeFilter, setTimeFilter] = useState('all-time');
  const [currentPage, setCurrentPage] = useState(1);
  const [leaderboardData, setLeaderboardData] = useState([]);
  const currentUser = { username: 'PlayerOne' };

  useEffect(() => {
    const gameId = String(selectedGame);
    const baseData = mockLeaderboard[gameId] || mockLeaderboard['1'];

    const enrichedData = baseData.map((entry, index) => ({
      ...entry,
      change: Math.floor(Math.random() * 10) - 3,
      avatar: `https://picsum.photos/seed/${entry.username}/100/100`,
    }));

    for (let i = 0; i < 20; i++) {
      enrichedData.push({
        rank: baseData.length + i + 1,
        username: `Player${1000 + i}`,
        score: Math.max(1000000 - i * 50000, 10000),
        change: Math.floor(Math.random() * 5) - 2,
        avatar: `https://picsum.photos/seed/player${1000 + i}/100/100`,
      });
    }

    setLeaderboardData(enrichedData);
    setCurrentPage(1);
  }, [selectedGame, timeFilter]);

  const totalPages = Math.ceil(leaderboardData.length / ITEMS_PER_PAGE);
  const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
  const currentData = leaderboardData.slice(startIndex, startIndex + ITEMS_PER_PAGE);

  const userRank = leaderboardData.findIndex(e => e.username === currentUser.username) + 1;

  return (
    <Layout>
      <div className="leaderboard-page">
        <header className="page-header">
          <h1>Leaderboard</h1>
          <p>Compete with players worldwide</p>
        </header>

        <div className="leaderboard-controls">
          <div className="game-select">
            <label>Game:</label>
            <select value={selectedGame} onChange={(e) => setSelectedGame(Number(e.target.value))}>
              {MOCK_GAMES.map(game => (
                <option key={game.id} value={game.id}>{game.title}</option>
              ))}
            </select>
          </div>

          <div className="time-filters">
            {TIME_FILTERS.map(filter => (
              <button
                key={filter.key}
                className={`filter-btn ${timeFilter === filter.key ? 'active' : ''}`}
                onClick={() => setTimeFilter(filter.key)}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>

        {userRank > 0 && userRank > 3 && (
          <div className="your-rank-banner">
            <span>Your rank: #{userRank}</span>
          </div>
        )}

        <div className="leaderboard-container">
          <div className="leaderboard-header">
            <span className="col-rank">Rank</span>
            <span className="col-player">Player</span>
            <span className="col-score">Score</span>
            <span className="col-change">Change</span>
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
          <div className="pagination">
            <button
              className="btn btn-secondary"
              disabled={currentPage === 1}
              onClick={() => setCurrentPage(p => p - 1)}
            >
              Previous
            </button>
            <span className="page-info">
              Page {currentPage} of {totalPages}
            </span>
            <button
              className="btn btn-secondary"
              disabled={currentPage === totalPages}
              onClick={() => setCurrentPage(p => p + 1)}
            >
              Next
            </button>
          </div>
        )}
      </div>
    </Layout>
  );
}