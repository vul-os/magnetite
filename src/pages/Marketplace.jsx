import { useState, useMemo } from 'react';
import GameCard from '../components/GameCard';
import Layout from '../components/Layout';
import OnboardingTour from '../components/OnboardingTour';
import { useTour } from '../hooks/useTour';
import './Marketplace.css';

const TOUR_KEY = 'magnetite_marketplace_tour_done';

const MARKETPLACE_TOUR_STEPS = [
  {
    targetSelector: '.search-container',
    title: 'Search Games',
    description: 'Find games by name or browse through our collection of titles from independent developers.',
    position: 'bottom',
  },
  {
    targetSelector: '.filter-group:first-child',
    title: 'Filter by Category',
    description: 'Narrow down your search by selecting specific game categories like Action, Puzzle, Racing, and more.',
    position: 'bottom',
  },
  {
    targetSelector: '.filter-group:nth-child(2)',
    title: 'Price Range',
    description: 'Set your preferred price range to find games that fit your budget.',
    position: 'bottom',
  },
  {
    targetSelector: '.filter-group:nth-child(3)',
    title: 'Sort Results',
    description: 'Sort games by popularity, newest releases, price, or rating to find exactly what you want.',
    position: 'bottom',
  },
  {
    targetSelector: '.game-grid',
    title: 'Browse Games',
    description: 'Explore our collection of games. Click on any game card to view details and start playing!',
    position: 'top',
  },
];

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Raiders', developer: 'StarForge Studios', fee_per_session: 0.50, category: 'Action', thumbnail: 'https://picsum.photos/seed/game1/400/225', rating: 4.5, players_online: 234, is_new: false },
  { id: 2, title: 'Puzzle Dimension', developer: 'MindBend Games', fee_per_session: 0.25, category: 'Puzzle', thumbnail: 'https://picsum.photos/seed/game2/400/225', rating: 4.8, players_online: 89, is_new: true },
  { id: 3, title: 'Speed Legends', developer: 'Velocity Labs', fee_per_session: 0.75, category: 'Racing', thumbnail: 'https://picsum.photos/seed/game3/400/225', rating: 4.2, players_online: 156, is_new: false },
  { id: 4, title: 'Dungeon Depths', developer: 'Tome Interactive', fee_per_session: 1.00, category: 'RPG', thumbnail: 'https://picsum.photos/seed/game4/400/225', rating: 4.9, players_online: 412, is_new: false },
  { id: 5, title: 'Strategy Command', developer: 'Tactical Soft', fee_per_session: 0.40, category: 'Strategy', thumbnail: 'https://picsum.photos/seed/game5/400/225', rating: 4.4, players_online: 67, is_new: true },
  { id: 6, title: 'Retro Arcade', developer: 'Pixel Dreams', fee_per_session: 0.15, category: 'Arcade', thumbnail: 'https://picsum.photos/seed/game6/400/225', rating: 4.1, players_online: 23, is_new: false },
  { id: 7, title: 'Cyber Assault', developer: 'Neon Forge', fee_per_session: 0.60, category: 'Action', thumbnail: 'https://picsum.photos/seed/game7/400/225', rating: 4.6, players_online: 198, is_new: true },
  { id: 8, title: 'Word Master', developer: 'Lexicon Labs', fee_per_session: 0.20, category: 'Puzzle', thumbnail: 'https://picsum.photos/seed/game8/400/225', rating: 4.3, players_online: 45, is_new: false },
  { id: 9, title: 'Turbo Drift', developer: 'Road Warriors', fee_per_session: 0.55, category: 'Racing', thumbnail: 'https://picsum.photos/seed/game9/400/225', rating: 4.7, players_online: 178, is_new: false },
  { id: 10, title: 'Dragon Quest', developer: 'Mythic Entertainment', fee_per_session: 1.25, category: 'RPG', thumbnail: 'https://picsum.photos/seed/game10/400/225', rating: 4.9, players_online: 523, is_new: false },
  { id: 11, title: 'Empire Builder', developer: 'Sovereign Games', fee_per_session: 0.45, category: 'Strategy', thumbnail: 'https://picsum.photos/seed/game11/400/225', rating: 4.5, players_online: 112, is_new: true },
  { id: 12, title: 'Space Invaders', developer: 'RetroCore', fee_per_session: 0.10, category: 'Arcade', thumbnail: 'https://picsum.photos/seed/game12/400/225', rating: 4.0, players_online: 34, is_new: false },
];

const CATEGORIES = ['All', 'Action', 'Puzzle', 'Racing', 'RPG', 'Strategy', 'Arcade'];
const SORT_OPTIONS = [
  { value: 'popular', label: 'Most Popular' },
  { value: 'new', label: 'Newest' },
  { value: 'price-low', label: 'Price: Low to High' },
  { value: 'price-high', label: 'Price: High to Low' },
  { value: 'rating', label: 'Highest Rated' },
];

function LoadingSkeleton() {
  return (
    <div className="game-grid">
      {[...Array(6)].map((_, i) => (
        <GameCard key={i} game={null} loading />
      ))}
    </div>
  );
}

function EmptyState({ hasFilters, onClearFilters }) {
  return (
    <div className="empty-state">
      <div className="empty-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
          <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
        </svg>
      </div>
      <h3>No games found</h3>
      <p>{hasFilters ? "Try adjusting your filters to find what you're looking for" : "Check back soon for new games"}</p>
      {hasFilters && (
        <button className="clear-filters-btn" onClick={onClearFilters}>
          Clear All Filters
        </button>
      )}
    </div>
  );
}

export default function Marketplace() {
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('All');
  const [priceRange, setPriceRange] = useState([0, 5]);
  const [sortBy, setSortBy] = useState('popular');
  const [isLoading] = useState(false);

  const tour = useTour(MARKETPLACE_TOUR_STEPS, {
    onComplete: () => localStorage.setItem(TOUR_KEY, 'true'),
    onSkip: () => localStorage.setItem(TOUR_KEY, 'true'),
  });

  const shouldStartTour = !localStorage.getItem(TOUR_KEY);

  if (shouldStartTour && !tour.isActive) {
    setTimeout(() => tour.start(), 500);
  }

  const hasActiveFilters = category !== 'All' || search !== '' || priceRange[0] > 0 || priceRange[1] < 5;

  const filteredGames = useMemo(() => {
    let games = MOCK_GAMES.filter(game => {
      const matchesSearch = game.title.toLowerCase().includes(search.toLowerCase()) ||
                           game.developer.toLowerCase().includes(search.toLowerCase());
      const matchesCategory = category === 'All' || game.category === category;
      const matchesPrice = game.fee_per_session >= priceRange[0] && game.fee_per_session <= priceRange[1];
      return matchesSearch && matchesCategory && matchesPrice;
    });

    switch (sortBy) {
      case 'new':
        games = [...games].sort((a, b) => (b.is_new ? 1 : 0) - (a.is_new ? 1 : 0));
        break;
      case 'price-low':
        games = [...games].sort((a, b) => a.fee_per_session - b.fee_per_session);
        break;
      case 'price-high':
        games = [...games].sort((a, b) => b.fee_per_session - a.fee_per_session);
        break;
      case 'rating':
        games = [...games].sort((a, b) => b.rating - a.rating);
        break;
      case 'popular':
      default:
        games = [...games].sort((a, b) => b.players_online - a.players_online);
        break;
    }

    return games;
  }, [search, category, priceRange, sortBy]);

  const clearAllFilters = () => {
    setSearch('');
    setCategory('All');
    setPriceRange([0, 5]);
    setSortBy('popular');
  };

  const removeFilter = (filterType) => {
    switch (filterType) {
      case 'search':
        setSearch('');
        break;
      case 'category':
        setCategory('All');
        break;
      case 'price':
        setPriceRange([0, 5]);
        break;
    }
  };

  return (
    <Layout>
      <div className="marketplace">
        <header className="marketplace-header">
          <div className="header-content">
            <h1>Marketplace</h1>
            <p className="header-subtitle">Discover and play amazing games from independent developers</p>
            <div className="search-container">
              <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
              </svg>
              <input
                type="text"
                placeholder="Search games or developers..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="search-input"
              />
              {search && (
                <button className="search-clear" onClick={() => setSearch('')}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
            </div>
          </div>
        </header>

        <div className="filters-section">
          <div className="filters-bar">
            <div className="filter-group">
              <label className="filter-label">Category</label>
              <select
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                className="filter-select"
              >
                {CATEGORIES.map(cat => (
                  <option key={cat} value={cat}>{cat}</option>
                ))}
              </select>
            </div>

            <div className="filter-group">
              <label className="filter-label">Price Range</label>
              <div className="price-range-container">
                <span className="price-value">${priceRange[0].toFixed(2)} - ${priceRange[1].toFixed(2)}</span>
                <input
                  type="range"
                  min="0"
                  max="5"
                  step="0.25"
                  value={priceRange[1]}
                  onChange={(e) => setPriceRange([priceRange[0], parseFloat(e.target.value)])}
                  className="price-slider"
                />
              </div>
            </div>

            <div className="filter-group">
              <label className="filter-label">Sort By</label>
              <select
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value)}
                className="filter-select"
              >
                {SORT_OPTIONS.map(option => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </div>
          </div>

          {hasActiveFilters && (
            <div className="active-filters">
              <span className="active-filters-label">Active filters:</span>
              {search && (
                <button className="filter-tag" onClick={() => removeFilter('search')}>
                  "{search}"
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              {category !== 'All' && (
                <button className="filter-tag" onClick={() => removeFilter('category')}>
                  {category}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              {(priceRange[0] > 0 || priceRange[1] < 5) && (
                <button className="filter-tag" onClick={() => removeFilter('price')}>
                  ${priceRange[0].toFixed(2)} - ${priceRange[1].toFixed(2)}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              <button className="clear-all-btn" onClick={clearAllFilters}>
                Clear all
              </button>
            </div>
          )}
        </div>

        <div className="marketplace-content">
          <div className="results-header">
            <span className="results-count">{filteredGames.length} game{filteredGames.length !== 1 ? 's' : ''} found</span>
          </div>

          {isLoading ? (
            <LoadingSkeleton />
          ) : filteredGames.length > 0 ? (
            <div className="game-grid">
              {filteredGames.map(game => (
                <GameCard key={game.id} game={game} showPlayButton />
              ))}
            </div>
          ) : (
            <EmptyState hasFilters={hasActiveFilters} onClearFilters={clearAllFilters} />
          )}
        </div>
      </div>
      <OnboardingTour
        steps={MARKETPLACE_TOUR_STEPS}
        currentStep={tour.currentStep}
        isActive={tour.isActive}
        isFirst={tour.isFirst}
        isLast={tour.isLast}
        onNext={tour.next}
        onBack={tour.back}
        onSkip={tour.skip}
      />
    </Layout>
  );
}
