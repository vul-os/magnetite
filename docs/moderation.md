# Review Moderation

Magnetite provides a review-reporting pipeline for players to flag inappropriate game reviews. Admin moderators can then list and dismiss flagged reports via the admin API.

## User-Facing Flow

Players submit reports via `POST /api/v1/games/:game_id/reviews/:review_id/report`:

```json
{
  "reason": "spam",
  "description": "This review is advertising a competing product."
}
```

The report is stored in the `review_reports` table. Valid reasons include: `spam`, `inappropriate`, `fake_review`, `off_topic`, `harassment`.

## Admin Moderation Surface

Admin endpoints (requires admin role):

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/admin/review-reports` | List reports (paginated; filter by `?reason=`, `?status=`) |
| `POST` | `/api/v1/admin/review-reports/:id/dismiss` | Dismiss a report (no action on the review) |
| `POST` | `/api/v1/admin/review-reports/:id/action` | Take action: `{ action: "remove_review" }` removes the review |

### Query parameters for listing

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | int | 20 | Page size (max 100) |
| `offset` | int | 0 | Page offset |
| `reason` | string | — | Filter by report reason |
| `status` | string | `pending` | `pending` \| `dismissed` \| `actioned` |

### Response shape

```json
{
  "reports": [
    {
      "id": "uuid",
      "review_id": "uuid",
      "reporter_id": "uuid",
      "reason": "spam",
      "description": "...",
      "status": "pending",
      "created_at": "2026-06-01T00:00:00Z"
    }
  ],
  "total": 5,
  "limit": 20,
  "offset": 0
}
```

## Implementation Notes

- Reports go into `review_reports` via `reviews::report_review` (already wired at `POST /api/v1/games/:id/reviews/:id/report`).
- The admin listing and action endpoints are in `admin.rs` (`GET /admin/review-reports`, `POST /admin/review-reports/:id/dismiss`, `POST /admin/review-reports/:id/action`).
- Without admin action the report accumulates silently — the moderation endpoints close this gap.

## Known Gaps (Bucket D)

- Email notification to the reporter when their report is actioned is not yet wired.
- Appeals by the reviewed developer are not yet implemented.
