# Blocking and Unblocking Users

Magnetite provides a user-level block system that prevents unwanted interactions between players.

## Behavior

When user A blocks user B:

- Friend requests between A and B are rejected (both directions).
- Game invites between A and B are rejected.
- Blocked users do not appear in friend search results for the blocker.
- The block is one-way: A blocks B, but B does not automatically block A.

## Endpoints

### Block a user

```
POST /api/v1/friends/block/:id
Authorization: Bearer <jwt>
```

Returns `200 OK` or:
- `400 Bad Request` — when attempting to block yourself.
- `400 Bad Request` — when the user is already blocked.

### Unblock a user

```
DELETE /api/v1/friends/block/:id
Authorization: Bearer <jwt>
```

Returns `200 OK` with `{ message: "User unblocked" }`.

### List blocked users

```
GET /api/v1/friends/blocked
Authorization: Bearer <jwt>
```

Returns an array of blocked user summaries:

```json
[
  {
    "user_id": "uuid",
    "username": "bad_actor",
    "avatar_url": null
  }
]
```

## Frontend API

The `api.social` client surface includes:

```js
api.social.blockUser(userId)      // POST /api/v1/friends/block/:id
api.social.unblockUser(userId)    // DELETE /api/v1/friends/block/:id
api.social.blockedUsers()         // GET /api/v1/friends/blocked
```

## Backend Implementation

Block/unblock logic lives in `backend/src/api/social.rs` (`block_user`, `unblock_user`, `list_blocked_users`).  The underlying data is stored in the `blocked_users` table.

The service layer (`backend/src/services/friends.rs`) exposes `FriendService::block`, `FriendService::unblock`, and `FriendService::is_blocked` for use by other features that need to check the block state (e.g., game invite delivery).
