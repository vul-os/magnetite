import { useState, useMemo } from 'react';
import GameCard from '../components/GameCard';
import Layout from '../components/Layout';
import OnboardingTour from '../components/OnboardingTour';
import { useTour } from '../hooks/useTour';
import { useGames } from '../hooks/useGames';
import { useTranslation } from '../i18n/useTranslation';
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

const CATEGORIES = ['All', 'Action', 'Puzzle', 'Racing', 'RPG', 'Strategy', 'Arcade'];

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
  const { t } = useTranslation();
  return (
    <div className="empty-state">
      <div className="empty-icon" aria-hidden="true">
        <svg viewBox="0 0 64 64" fill="none">
          <circle cx="28" cy="28" r="20" stroke="currentColor" strokeWidth="1.5" opacity="0.4"/>
          <path d="M42 42L56 56" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" opacity="0.4"/>
          <path d="M22 28h12M28 22v12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" opacity="0.25"/>
        </svg>
      </div>
      <h3 className="empty-title">{t('store.noGamesFound')}</h3>
      <p className="empty-desc">
        {hasFilters
          ? t('store.noGamesFiltered')
          : t('store.noGamesEmpty')}
      </p>
      {hasFilters && (
        <button className="clear-filters-btn" onClick={onClearFilters}>
          {t('store.clearAllFilters')}
        </button>
      )}
    </div>
  );
}

function ErrorState({ message, onRetry }) {
  const { t } = useTranslation();
  return (
    <div className="empty-state" role="alert">
      <div className="empty-icon" aria-hidden="true">
        <svg viewBox="0 0 64 64" fill="none">
          <circle cx="32" cy="32" r="28" stroke="var(--color-error)" strokeWidth="1.5" opacity="0.5"/>
          <path d="M32 20v16M32 42v2" stroke="var(--color-error)" strokeWidth="2" strokeLinecap="round" opacity="0.7"/>
        </svg>
      </div>
      <h3 className="empty-title">{t('store.failedToLoad')}</h3>
      <p className="empty-desc">{message || t('store.couldNotConnect')}</p>
      {onRetry && (
        <button className="clear-filters-btn" onClick={onRetry}>
          {t('store.retry')}
        </button>
      )}
    </div>
  );
}

export default function Marketplace() {
  const { t } = useTranslation();
  const [search, setSearch]         = useState('');
  const [category, setCategory]     = useState('All');
  const [priceRange, setPriceRange] = useState([0, 5]);
  const [sortBy, setSortBy]         = useState('popular');

  const { games: allGames, loading: isLoading, error: gamesError } = useGames();

  const tour = useTour(MARKETPLACE_TOUR_STEPS, {
    onComplete: () => localStorage.setItem(TOUR_KEY, 'true'),
    onSkip:     () => localStorage.setItem(TOUR_KEY, 'true'),
  });

  const shouldStartTour = !localStorage.getItem(TOUR_KEY);
  if (shouldStartTour && !tour.isActive) {
    setTimeout(() => tour.start(), 500);
  }

  const hasActiveFilters =
    category !== 'All' || search !== '' || priceRange[0] > 0 || priceRange[1] < 5;

  // Honest, derived-from-the-catalogue stats — no invented numbers. Each is a
  // fold over exactly the games the page is showing.
  const catalogueStats = useMemo(() => {
    const totalOnline = allGames.reduce((s, g) => s + (g.players_online ?? 0), 0);
    const freeCount = allGames.filter((g) => {
      const fee = Number(g.fee_per_session ?? g.feePerSession);
      const paid = Number.isFinite(fee) && fee > 0 && !(g.is_free ?? g.isFree);
      return !paid;
    }).length;
    return { games: allGames.length, online: totalOnline, free: freeCount };
  }, [allGames]);

  const filteredGames = useMemo(() => {
    let games = allGames.filter(game => {
      const matchesSearch   = game.title.toLowerCase().includes(search.toLowerCase()) ||
                              game.developer.toLowerCase().includes(search.toLowerCase());
      const matchesCategory = category === 'All' || game.category === category;
      const matchesPrice    = game.fee_per_session >= priceRange[0] && game.fee_per_session <= priceRange[1];
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
  }, [allGames, search, category, priceRange, sortBy]);

  const clearAllFilters = () => {
    setSearch('');
    setCategory('All');
    setPriceRange([0, 5]);
    setSortBy('popular');
  };

  const removeFilter = (filterType) => {
    switch (filterType) {
      case 'search':   setSearch(''); break;
      case 'category': setCategory('All'); break;
      case 'price':    setPriceRange([0, 5]); break;
      default: break;
    }
  };

  return (
    <Layout>
      <div className="marketplace">
        {/* ── Atmospheric header ─────────────────────────────────────────── */}
        <header className="marketplace-header bg-field" data-aura aria-labelledby="marketplace-heading">
          <div className="field-aura" aria-hidden="true" />
          <svg className="mkt-field-lines" viewBox="0 0 1440 520" fill="none" aria-hidden="true" preserveAspectRatio="xMidYMid slice">
            <g stroke="var(--field)" strokeLinecap="round">
              <path d="M-40 360 C 360 180, 1080 180, 1480 360" opacity=".16" strokeWidth="1.2" />
              <path d="M-40 300 C 380 90, 1060 90, 1480 300" opacity=".12" strokeWidth="1.2" />
              <path className="flowline" d="M-40 330 C 370 130, 1070 130, 1480 330" opacity=".4" strokeWidth="1.4" />
              <path d="M-40 240 C 400 20, 1040 20, 1480 240" opacity=".08" strokeWidth="1.2" />
            </g>
          </svg>

          <div className="header-content reveal">
            <span className="kicker reveal-1">{t('store.marketplaceKicker')}</span>
            <h1 id="marketplace-heading" className="mkt-heading reveal-2">
              {t('store.marketplaceHeading')}
            </h1>
            <p className="header-subtitle reveal-3">
              Browser-native via WASM. Server-authoritative Rust netcode.<br />
              Independent developers. Real USDC earnings.
            </p>

            {/* Search */}
            <div className="search-container reveal-4" role="search">
              <label htmlFor="marketplace-search" className="visually-hidden">{t('store.searchLabel')}</label>
              <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
              </svg>
              <input
                type="text"
                id="marketplace-search"
                placeholder={t('store.searchPlaceholder')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="search-input"
                aria-label={t('store.searchLabel')}
              />
              {search && (
                <button
                  className="search-clear"
                  onClick={() => setSearch('')}
                  aria-label={t('store.clearSearch')}
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
            </div>

            {/* Category pills */}
            <nav className="category-pills reveal-5" aria-label={t('store.categoriesLabel')}>
              {CATEGORIES.map(cat => (
                <button
                  key={cat}
                  className={`category-pill ${category === cat ? 'active' : ''}`}
                  onClick={() => setCategory(cat)}
                  aria-pressed={category === cat}
                >
                  {cat}
                </button>
              ))}
            </nav>

            {/* Live catalogue readout — every figure is folded from the games
                actually loaded, never invented. */}
            {!isLoading && !gamesError && catalogueStats.games > 0 && (
              <dl className="mkt-stats reveal-6" aria-label="Catalogue at a glance">
                <div className="mkt-stat">
                  <dt>Titles</dt>
                  <dd>{catalogueStats.games}</dd>
                </div>
                <div className="mkt-stat">
                  <dt>Players online</dt>
                  <dd className="mkt-stat-live">
                    <span className="mkt-stat-dot pulse-live" aria-hidden="true" />
                    {catalogueStats.online.toLocaleString()}
                  </dd>
                </div>
                <div className="mkt-stat">
                  <dt>Free to play</dt>
                  <dd>{catalogueStats.free}</dd>
                </div>
              </dl>
            )}
          </div>
        </header>

        {/* ── Sticky filter bar ──────────────────────────────────────────── */}
        <div className="filters-section" role="region" aria-label={t('store.filterLabel')}>
          <div className="filters-bar">
            <div className="filter-group">
              <label className="filter-label" htmlFor="mp-price-range">
                {t('store.maxPrice')}
              </label>
              <div className="price-range-container">
                <span className="price-value" aria-live="polite">
                  {priceRange[0] > 0 ? `$${priceRange[0].toFixed(2)} – ` : 'Up to '}
                  ${priceRange[1].toFixed(2)} USDC
                </span>
                <input
                  type="range"
                  id="mp-price-range"
                  min="0"
                  max="5"
                  step="0.25"
                  value={priceRange[1]}
                  onChange={(e) => setPriceRange([priceRange[0], parseFloat(e.target.value)])}
                  className="mkt-price-slider"
                  aria-label={t('store.priceAriaLabel')}
                />
              </div>
            </div>

            <div className="filter-group">
              <label className="filter-label" htmlFor="mp-sort-by">{t('store.sortBy')}</label>
              <select
                id="mp-sort-by"
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value)}
                className="filter-select"
              >
                <option value="popular">{t('store.sortPopular')}</option>
                <option value="new">{t('store.sortNew')}</option>
                <option value="price-low">{t('store.sortPriceLow')}</option>
                <option value="price-high">{t('store.sortPriceHigh')}</option>
                <option value="rating">{t('store.sortRating')}</option>
              </select>
            </div>

            <div className="filter-results-info" role="status" aria-live="polite">
              <span className="results-count">
                <span className="results-num">{filteredGames.length}</span>
                {' '}{filteredGames.length !== 1 ? t('store.resultsPlural', { count: '' }).replace('{{count}} ', '') : t('store.results', { count: '' }).replace('{{count}} ', '')}
              </span>
            </div>
          </div>

          {hasActiveFilters && (
            <div className="active-filters" role="region" aria-label={t('store.activeFilters')}>
              <span className="active-filters-label">{t('store.activeLabel')}</span>
              {search && (
                <button
                  className="filter-tag"
                  onClick={() => removeFilter('search')}
                  aria-label={t('store.removeFilter', { type: 'search', value: search })}
                >
                  &ldquo;{search}&rdquo;
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              {category !== 'All' && (
                <button
                  className="filter-tag"
                  onClick={() => removeFilter('category')}
                  aria-label={t('store.removeFilter', { type: 'category', value: category })}
                >
                  {category}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              {(priceRange[0] > 0 || priceRange[1] < 5) && (
                <button
                  className="filter-tag"
                  onClick={() => removeFilter('price')}
                  aria-label={t('store.removePriceFilter')}
                >
                  ${priceRange[0].toFixed(2)}–${priceRange[1].toFixed(2)}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
              <button className="clear-all-btn" onClick={clearAllFilters}>
                {t('store.clearAll')}
              </button>
            </div>
          )}
        </div>

        {/* ── Game grid ──────────────────────────────────────────────────── */}
        <div className="marketplace-content">
          {isLoading ? (
            <LoadingSkeleton />
          ) : gamesError ? (
            <ErrorState message={gamesError} />
          ) : filteredGames.length > 0 ? (
            <div className="game-grid sr sr-group">
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
