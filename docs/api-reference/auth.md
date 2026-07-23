# Authentication Endpoints

## POST /api/v1/auth/register

Create a new user account.

### Request

```json
{
  "email": "player@example.com",
  "username": "player1",
  "password": "securePassword123"
}
```

### Response

```json
{
  "success": true,
  "data": {
    "user_id": "usr_a1b2c3",
    "email": "player@example.com",
    "username": "player1",
    "created_at": "2024-01-15T10:30:00Z"
  },
  "meta": {
    "request_id": "req_abc123",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| email | string | Yes | Valid email format |
| username | string | Yes | 3-20 chars, alphanumeric |
| password | string | Yes | Min 8 chars, 1 uppercase, 1 number |

### Error Codes

| Code | Message |
|------|---------|
| EMAIL_TAKEN | Email already registered |
| USERNAME_TAKEN | Username already taken |

---

## POST /api/v1/auth/login

Authenticate and receive access token.

### Request

```json
{
  "username": "player1",
  "password": "securePassword123",
  "totp_code": "123456"
}
```

`username` and `password` are required. `totp_code` is only required when the
account has TOTP 2FA enabled; omit it otherwise.

### Response

```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g...",
    "expires_in": 3600,
    "token_type": "Bearer"
  },
  "meta": {
    "request_id": "req_abc123",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### Tokens

| Token | Lifetime | Purpose |
|-------|----------|---------|
| access_token | 1 hour | API authentication |
| refresh_token | 7 days | Renew access token |

---

## POST /api/v1/auth/refresh

Refresh expired access token.

### Request

```json
{
  "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g..."
}
```

### Response

```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "bmV3IHJlZnJlc2ggdG9rZW4...",
    "expires_in": 3600
  }
}
```

---

## GET /api/v1/auth/me

Get current authenticated user.

### Headers

```
Authorization: Bearer <access_token>
```

### Response

```json
{
  "success": true,
  "data": {
    "user_id": "usr_a1b2c3",
    "email": "player@example.com",
    "username": "player1",
    "wallet": {
      "balance": 1500,
      "currency": "MGNT"
    },
    "stats": {
      "games_played": 42,
      "games_won": 28,
      "win_rate": 0.667
    },
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

---

## POST /api/v1/auth/logout

Invalidate current session.

### Request

```json
{
  "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g..."
}
```

### Response

```json
{
  "success": true,
  "data": {
    "message": "Successfully logged out"
  }
}
```

---

## POST /api/v1/auth/verify-email

Verify user email address.

### Request

```json
{
  "token": "e3b0c44298fc1c149afbf4c8996fb924"
}
```

### Response

```json
{
  "success": true,
  "data": {
    "verified": true
  }
}
```
