# Environment Variables Reference

Complete reference for all Magnetite environment variables.

## Required Variables

### DATABASE_URL

PostgreSQL connection string.

```bash
DATABASE_URL=postgresql://user:password@host:5432/database
```

| Component | Description |
|-----------|-------------|
| `user` | Database username |
| `password` | Database password |
| `host` | Database hostname |
| `port` | PostgreSQL port (default: 5432) |
| `database` | Database name |

Example:
```bash
DATABASE_URL=postgresql://magnetite:secure_password@db.example.com:5432/magnetite
```

### JWT_SECRET

Secret key for signing JWT tokens. Must be at least 32 characters.

```bash
# Generate a secure secret
openssl rand -hex 32
```

```bash
JWT_SECRET=your-secure-secret-at-least-32-characters-long
```

**Security Note**: Use a cryptographically secure random string. Never commit this to version control.

## Server Configuration

### SERVER_HOST

Interface the backend binds to.

```bash
SERVER_HOST=0.0.0.0
```

| Value | Description |
|-------|-------------|
| `0.0.0.0` | Bind to all interfaces |
| `127.0.0.1` | Bind to localhost only |
| `::` | Bind to all IPv6 interfaces |

### SERVER_PORT

Port for the backend server.

```bash
SERVER_PORT=8080
```

### RUST_LOG

Logging level for Rust backend.

```bash
RUST_LOG=info
```

| Level | Description |
|-------|-------------|
| `trace` | Very detailed logs |
| `debug` | Debug information |
| `info` | General information (default) |
| `warn` | Warning messages |
| `error` | Error messages |

## PostgreSQL Configuration

### POSTGRES_USER

PostgreSQL username.

```bash
POSTGRES_USER=magnetite
```

### POSTGRES_PASSWORD

PostgreSQL password.

```bash
POSTGRES_PASSWORD=your-secure-password
```

### POSTGRES_DB

PostgreSQL database name.

```bash
POSTGRES_DB=magnetite
```

### POSTGRES_HOST

PostgreSQL hostname (for Docker Compose).

```bash
POSTGRES_HOST=postgres
```

### POSTGRES_PORT

PostgreSQL port.

```bash
POSTGRES_PORT=5432
```

## Redis Configuration

### REDIS_URL

Redis connection string (optional).

```bash
REDIS_URL=redis://redis:6379
```

## Payment Integration

### CIRCLE_API_KEY

Circle API key for payment processing.

```bash
CIRCLE_API_KEY=your_circle_api_key
```

Get from: https://circle.com/

### PAYSTACK_SECRET_KEY

Paystack secret key for African payment processing.

```bash
PAYSTACK_SECRET_KEY=your_paystack_secret_key
```

Get from: https://paystack.com/

### SUBSCRIPTION_WEBHOOK_SECRET

Webhook secret for subscription payment verification.

```bash
SUBSCRIPTION_WEBHOOK_SECRET=your_webhook_secret
```

## OAuth Configuration (Optional)

### GOOGLE_CLIENT_ID

Google OAuth client ID for login.

```bash
GOOGLE_CLIENT_ID=your_google_client_id.apps.googleusercontent.com
```

### GOOGLE_CLIENT_SECRET

Google OAuth client secret.

```bash
GOOGLE_CLIENT_SECRET=your_google_client_secret
```

### DISCORD_CLIENT_ID

Discord OAuth client ID.

```bash
DISCORD_CLIENT_ID=your_discord_client_id
```

### DISCORD_CLIENT_SECRET

Discord OAuth client secret.

```bash
DISCORD_CLIENT_SECRET=your_discord_client_secret
```

### GITHUB_CLIENT_ID

GitHub OAuth client ID.

```bash
GITHUB_CLIENT_ID=your_github_client_id
```

### GITHUB_CLIENT_SECRET

GitHub OAuth client secret.

```bash
GITHUB_CLIENT_SECRET=your_github_client_secret
```

## Email Configuration

### SMTP_HOST

SMTP server hostname.

```bash
SMTP_HOST=smtp.example.com
```

### SMTP_PORT

SMTP server port.

```bash
SMTP_PORT=587
```

### SMTP_USERNAME

SMTP username.

```bash
SMTP_USERNAME=your_smtp_username
```

### SMTP_PASSWORD

SMTP password.

```bash
SMTP_PASSWORD=your_smtp_password
```

### SMTP_FROM

From address for emails.

```bash
SMTP_FROM=noreply@example.com
```

## AWS Configuration (Optional)

### AWS_ACCESS_KEY_ID

AWS access key for SES email.

```bash
AWS_ACCESS_KEY_ID=your_access_key
```

### AWS_SECRET_ACCESS_KEY

AWS secret access key.

```bash
AWS_SECRET_ACCESS_KEY=your_secret_key
```

### AWS_REGION

AWS region for SES.

```bash
AWS_REGION=us-east-1
```

## Application Settings

### APP_URL

Public URL of the application.

```bash
APP_URL=https://magnetite.example.com
```

### CORS_ORIGINS

Allowed CORS origins (comma-separated).

```bash
CORS_ORIGINS=https://magnetite.example.com,https://www.magnetite.example.com
```

### SESSION_DURATION_HOURS

Session duration in hours.

```bash
SESSION_DURATION_HOURS=168
```

## Example .env File

```bash
# Required
DATABASE_URL=postgresql://magnetite:change_me_in_production@postgres:5432/magnetite
JWT_SECRET=change_this_to_a_secure_random_string_at_least_32_chars

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
RUST_LOG=info

# PostgreSQL
POSTGRES_USER=magnetite
POSTGRES_PASSWORD=change_me_in_production
POSTGRES_DB=magnetite

# Redis (optional)
REDIS_URL=redis://redis:6379

# Payments (optional)
# CIRCLE_API_KEY=
# PAYSTACK_SECRET_KEY=

# OAuth (optional)
# GOOGLE_CLIENT_ID=
# GOOGLE_CLIENT_SECRET=
# DISCORD_CLIENT_ID=
# DISCORD_CLIENT_SECRET=
# GITHUB_CLIENT_ID=
# GITHUB_CLIENT_SECRET=

# Email (optional)
# SMTP_HOST=smtp.example.com
# SMTP_PORT=587
# SMTP_USERNAME=
# SMTP_PASSWORD=
# SMTP_FROM=noreply@example.com

# AWS SES (optional)
# AWS_ACCESS_KEY_ID=
# AWS_SECRET_ACCESS_KEY=
# AWS_REGION=us-east-1

# Application
APP_URL=https://magnetite.example.com
CORS_ORIGINS=https://magnetite.example.com
SESSION_DURATION_HOURS=168
```

## Variable Priority

1. Environment variables set at runtime (highest)
2. Variables in `.env` file
3. Default values in code (lowest)

## Validation

Magnetite will fail to start if required variables are missing:

- `DATABASE_URL` - Required
- `JWT_SECRET` - Required

Optional variables have sensible defaults and will not cause startup failures if omitted.
