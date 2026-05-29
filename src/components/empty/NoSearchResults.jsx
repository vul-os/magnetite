import EmptyState from './EmptyState';

const SearchIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.35-4.35" />
    <path d="M8.5 8.5l7 7" opacity="0.5" />
    <circle cx="11" cy="11" r="4" opacity="0.3" />
  </svg>
);

export default function NoSearchResults({ query, action }) {
  return (
    <EmptyState
      icon={<SearchIcon />}
      title="No results found"
      description={`We couldn't find any matches for "${query || 'your search'}". Try adjusting your search terms or filters.`}
      action={action}
    />
  );
}
