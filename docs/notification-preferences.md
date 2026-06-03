# Notification Preferences

Magnetite lets users control which notification categories they receive and
through which delivery channels. Preferences are stored per-user in the
database and are applied by the notification delivery layer before dispatching
email, in-app, or push messages.

---

## Data Model

Preferences live in the `notification_preferences` table. Each user has at most
one row; the row is created with safe defaults on first access (upsert-on-read).

### Categories

| Category | Description |
|---|---|
| `payouts` | Payout completions and subscription renewals |
| `social` | Friend requests and game invites |
| `achievements` | Achievement unlocks and milestone rewards |
| `marketing` | News, promotions, and platform announcements |

### Channels

| Channel | Description |
|---|---|
| `email` | Transactional email via Resend or SES |
| `in_app` | Badge count + in-app notification feed |
| `push` | Browser / device push notification (future) |

### Columns

The table has one boolean column per `{category}_{channel}` combination — 12
columns in total:

```
payouts_email    payouts_in_app    payouts_push
social_email     social_in_app     social_push
achievements_email  achievements_in_app  achievements_push
marketing_email  marketing_in_app  marketing_push
```

**Defaults:**

| Column | Default |
|---|---|
| `payouts_*` | `true` |
| `social_*` | `true` |
| `achievements_email` / `achievements_in_app` | `true` |
| `achievements_push` | `false` |
| `marketing_*` | `false` |

---

## REST API

### GET /api/v1/notifications/preferences

Returns the authenticated user's preferences. Creates a default row on first
call (upsert-on-read).

**Auth:** Bearer JWT required.

**Response `200 OK`:**

```json
{
  "id": "uuid",
  "user_id": "uuid",
  "payouts_email": true,
  "payouts_in_app": true,
  "payouts_push": true,
  "social_email": true,
  "social_in_app": true,
  "social_push": true,
  "achievements_email": true,
  "achievements_in_app": true,
  "achievements_push": false,
  "marketing_email": false,
  "marketing_in_app": false,
  "marketing_push": false,
  "created_at": "2026-06-01T00:00:00Z",
  "updated_at": "2026-06-01T00:00:00Z"
}
```

### PUT /api/v1/notifications/preferences

Partial update — only the fields present in the request body are written. All
fields are optional; omitted fields retain their current values.

**Auth:** Bearer JWT required.

**Request body (example):**

```json
{
  "marketing_email": false,
  "marketing_push": false,
  "achievements_push": true
}
```

**Response `200 OK`:** Same shape as `GET`, reflecting the updated state.

---

## Implementation

**Backend:** `backend/src/api/notifications.rs`

- `get_preferences` (line 681) — upserts a default row then returns it.
- `update_preferences` (line 703) — applies each supplied field via individual
  `UPDATE` statements wrapped in a macro; returns the final row.
- `channel_enabled(pool, user_id, category, channel)` (line 765) — called by
  notification dispatch code to skip a channel the user has disabled. The
  column name is constructed from `{category}_{channel}` and validated against
  an allowlist to prevent SQL injection.

Both handlers are mounted in `notifications::router()` at:

```
GET  /notifications/preferences
PUT  /notifications/preferences
```

which is nested at `/api/v1/notifications` in `main.rs` (line 111).

**Frontend:** `src/components/NotificationPreferences.jsx`

- Fetches `GET /api/v1/notifications/preferences` on mount via
  `api.notifications.getPreferences()`.
- Renders four `CategoryCard` sections (Payouts, Social, Achievements,
  Marketing), each with Email / In-App / Push toggle columns.
- On save, calls `PUT /api/v1/notifications/preferences` with the full current
  state.
- All UI copy resolved via `useTranslation()` from the `notifPrefs` namespace
  in `src/i18n/en.json`.
- Full accessibility: `role="switch"` toggles with `aria-checked`, visible
  focus rings, `<section>` / `<fieldset>` structure, `prefers-reduced-motion`
  support, tap targets ≥ 40 px.

**API client stubs** (`src/api/client.js`):

```js
api.notifications.getPreferences()          // GET /api/v1/notifications/preferences
api.notifications.updatePreferences(data)   // PUT /api/v1/notifications/preferences
```

---

## Wiring into a settings page

`NotificationPreferences` is a self-contained React component. To mount it:

```jsx
import NotificationPreferences from '../components/NotificationPreferences';

// Inside a settings tab:
<NotificationPreferences />
```

No route change is required. The component handles its own data fetching,
saving, and error states.

---

## Known gaps

- **Push channel** — `*_push` columns are stored and toggled but no push
  notification infrastructure (Service Worker, FCM, or APNs) is wired.
  The channel is reserved for a future pass.
- **Email dispatch respects preferences** — `channel_enabled()` is available
  for callers to check before sending email, but the notification dispatch
  code in `NotificationService` currently sends to all enabled channels without
  consulting preferences. A future pass will thread `channel_enabled()` checks
  into each notification helper.
