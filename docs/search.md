# Search

Magnetite provides full-text ranked game and user search via a PostgreSQL `tsvector`/`ts_rank` pipeline.

## Endpoint

```
GET /api/v1/search?q=<query>&search_type=<type>&limit=<n>&offset=<n>
                 &genre=<g>&category=<c>&min_rating=<r>&is_free=<bool>
```

All query parameters are optional except `q`.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `q` | string | required | Search query (multi-word supported) |
| `search_type` | `all` \| `games` \| `users` | `all` | Scope of the search |
| `limit` | int | 20 | Page size (max 100) |
| `offset` | int | 0 | Page offset |
| `genre` | string | — | Filter games by genre |
| `category` | string | — | Filter games by category |
| `min_rating` | float | — | Minimum average rating (0–5) |
| `is_free` | bool | — | `true` = free-only, `false` = paid-only |

## Response

```json
{
  "results": [
    {
      "result_type": "game",
      "id": "uuid",
      "title": "Oxide Arena",
      "description": "A top-down shooter",
      "developer_username": "alice"
    },
    {
      "result_type": "user",
      "id": "uuid",
      "username": "alice_dev",
      "avatar_url": null
    }
  ],
  "total": 42,
  "limit": 20,
  "offset": 0
}
```

## Ranking

Games are ranked by `ts_rank` using a GIN-indexed `search_vector` generated column on the `games` table. A higher rank means the query terms appear more prominently in the title and description.

The migration that adds `search_vector` is in `backend/migrations/` — the column is defined as:

```sql
search_vector tsvector GENERATED ALWAYS AS (
  to_tsvector('english', coalesce(title, '') || ' ' || coalesce(description, ''))
) STORED;
CREATE INDEX games_search_vector_gin ON games USING GIN(search_vector);
```

Legacy `ILIKE` scan is removed; all search goes through `plainto_tsquery('english', $1)`.

## Frontend Usage

```js
import { api } from 'src/api/client';

// Basic search
const results = await api.search.query('rust shooter');

// Typed + filtered
const filtered = await api.search.query(
  'arena',
  'games',     // search_type
  20,          // limit
  0,           // offset
  { genre: 'shooter', min_rating: 3.5, is_free: false }
);
```

The `useSearch` hook in `src/hooks/useSearch.js` wraps the client with:
- 300 ms debounce
- Recent-search history (localStorage, max 5)
- Structured error state on failure

## Known Gaps

- Geographic/device breakdown, store conversion funnel, and cohort analytics are not yet exposed.
- Full GIN migration must be applied to production DB for ranking to activate.
