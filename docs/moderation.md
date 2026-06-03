# Review Moderation

Magnetite provides a two-sided moderation pipeline for game reviews:

- **Players** report reviews they consider inappropriate.
- **Admins** list pending reports and take one of four actions: dismiss,
  remove the review, warn the author, or ban the author.

---

## User-facing reporting

`POST /api/v1/games/:game_id/reviews/:review_id/report`

**Auth:** Bearer JWT required.

**Request body:**

```json
{
  "reason": "spam"
}
```

Valid reason strings: `spam`, `inappropriate`, `fake_review`, `off_topic`,
`harassment`.

Reports are stored in the `review_reports` table. The call is idempotent —
submitting the same `(review_id, reporter_id, reason)` triple a second time
returns the existing report row (via `ON CONFLICT DO NOTHING`).

**Response `201 Created`:**

```json
{
  "id": "uuid",
  "review_id": "uuid",
  "reporter_id": "uuid",
  "reason": "spam",
  "created_at": "2026-06-01T00:00:00Z"
}
```

---

## Admin moderation surface

All endpoints below require an admin account (`is_admin = true`). The handlers
call `require_admin()` and are routed through `auth_middleware`.

### GET /api/v1/admin/review-reports

List pending (or filtered) reports with joined review and user metadata.

**Query parameters:**

| Param | Type | Default | Description |
|---|---|---|---|
| `page` | int | `1` | Page number (1-indexed) |
| `limit` | int | `20` | Page size (max 100) |
| `status` | string | `pending` | `pending` \| `dismissed` \| `resolved` |
| `reason` | string | — | Case-insensitive substring match on reason |

**Response `200 OK`:**

```json
{
  "data": [
    {
      "id": "uuid",
      "review_id": "uuid",
      "reporter_id": "uuid",
      "reporter_username": "alice",
      "review_author_id": "uuid",
      "review_author_username": "bob",
      "review_rating": 1,
      "review_content": "This game is terrible...",
      "reason": "spam",
      "status": "pending",
      "created_at": "2026-06-01T00:00:00Z"
    }
  ],
  "page": 1,
  "limit": 20,
  "total": 5,
  "total_pages": 1
}
```

### POST /api/v1/admin/review-reports/:id/action

Take a moderation action on a report.

**Request body:**

```json
{
  "action": "dismiss",
  "note": "Optional moderator note"
}
```

**Valid actions:**

| Action | Effect |
|---|---|
| `dismiss` | Marks the report as `dismissed`; the review is left intact |
| `remove_review` | Deletes the review (cascades to helpful votes); marks all reports for this review as `resolved` |
| `warn_user` | Sends the review author a system notification with the moderator note; marks the report as `resolved` |
| `ban_user` | Sets `users.banned_at = NOW()` on the review author; deletes the review; marks all reports as `resolved` |

**Response `204 No Content`** on success.

**Response `400 Bad Request`** if the `action` string is not one of the four
valid values.

**Response `404 Not Found`** if the report ID does not exist.

---

## Auto-flag

There is no automatic threshold-based flagging in the current implementation.
Reports are created only by explicit user action. A future pass may add a
trigger that auto-flags reviews with N or more pending reports.

---

## Implementation details

**Backend files:**

| File | Role |
|---|---|
| `backend/src/api/reviews.rs:362` | `report_review` — stores the report row |
| `backend/src/api/admin.rs:1389` | `list_review_reports` — paginated admin listing |
| `backend/src/api/admin.rs:1454` | `act_on_review_report` — four-action dispatcher |
| `backend/src/api/admin.rs:1722` | Router registration (`/review-reports`, `/review-reports/:id/action`) |

The router registration in `admin.rs:1722–1738` mounts both endpoints under
`/admin` (nested at `/api/v1/admin` in `main.rs`).

**Frontend:** The admin Finance and Users pages include the review-reports
table. Calls flow through the real backend — not mock data — unless
`VITE_USE_MOCKS=true`.

---

## Known gaps

- Email notification to the reporter when their report is actioned is not yet
  wired (`channel_enabled()` check is not in the dispatch path).
- Developer appeals for removed reviews are not implemented.
- Auto-flag on N-report threshold is not implemented.
