# Contributing to Magnetite

Thank you for your interest in contributing to Magnetite! This guide covers everything you need to know to get started, from setting up your development environment to understanding our release process.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Guide](#development-guide)
- [Testing Guide](#testing-guide)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)
- [Community](#community)

---

## Code of Conduct

We are committed to providing a welcoming and inclusive experience for all contributors. By participating in this project, you agree to uphold our standards of conduct.

### Our Standards

- **Be respectful and inclusive**: Treat everyone with respect. Discriminatory language or behavior based on race, gender, sexual orientation, disability, religion, or any other protected characteristic will not be tolerated.
- **Be constructive**: Provide feedback that helps others improve. Avoid personal attacks and hostile behavior.
- **Be collaborative**: Work together to build a positive community. Help newcomers and support diverse perspectives.
- **Be transparent**: Communicate openly about issues, decisions, and processes.

### Enforcement

Instances of abusive, harassing, or otherwise unacceptable behavior may be reported to the project maintainers. All complaints will be reviewed and investigated and may result in a response that is deemed necessary and appropriate.

---

## Getting Started

### Dev Environment Setup

#### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust | 1.75+ | Install via [rustup](https://rustup.rs) |
| Node.js | 18+ | LTS recommended |
| PostgreSQL | 15+ | Or use Docker |
| Docker | Latest | For local database services |

#### 1. Clone the Repository

```bash
git clone https://github.com/anomalyco/magnetite.git
cd magnetite
```

#### 2. Set Up Backend (Rust)

```bash
cd backend

# Build the project
cargo build

# Copy environment configuration
cp .env.example .env

# Edit .env with your database credentials:
# DATABASE_URL="postgres://postgres:postgres@localhost:5432/magnetite"
# JWT_SECRET="your-secret-key-here"
```

#### 3. Set Up Frontend (React)

```bash
# From project root
npm install
```

#### 4. Set Up Local Database (Docker)

```bash
# Start PostgreSQL and Redis containers
docker-compose up -d postgres redis

# Verify containers are running
docker-compose ps
```

#### 5. Run Migrations

```bash
cd backend/tools

# Apply all pending migrations
./migrate.sh up

# Check migration status
./migrate.sh status

# Reset database (WARNING: drops all data)
./migrate.sh reset
```

#### 6. Start Development Servers

```bash
# Terminal 1: Backend
cd backend
cargo run

# Terminal 2: Frontend
npm run dev
```

The frontend is available at `http://localhost:5173` and the backend API at `http://localhost:8080`.

### Repository Structure

```
magnetite/
├── backend/              # Rust backend (Axum)
│   ├── src/
│   │   ├── api/          # HTTP API handlers (auth, games, wallet, etc.)
│   │   ├── db/           # Database pool and connections
│   │   ├── services/     # Business logic layer
│   │   ├── middleware/   # Request middleware (CORS, rate limiting, logging)
│   │   ├── jobs/         # Background jobs
│   │   └── ws/           # WebSocket handlers
│   ├── migrations/       # SQL migrations
│   └── tools/            # Dev tools (migrate.sh)
├── src/                  # React frontend
│   ├── api/              # API client
│   ├── components/        # Reusable UI components
│   ├── context/           # React contexts
│   ├── data/              # Mock data for development
│   ├── hooks/             # Custom React hooks
│   ├── pages/             # Page components
│   └── assets/            # Static assets
├── e2e/                  # End-to-end tests (Playwright)
├── docs/                 # Documentation
└── scripts/              # Build and deployment scripts
```

### First PR Workflow

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/magnetite.git
   ```
3. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Make your changes** and commit following our commit message format
5. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```
6. **Open a Pull Request** against the `main` branch

---

## Development Guide

### Backend (Rust) Conventions

#### Code Style

- **Format code** before committing:
  ```bash
  cargo fmt
  ```
- **Check for issues** with Clippy:
  ```bash
  cargo clippy -- -D warnings
  ```
- Use `?` for error propagation instead of `.unwrap()` or `.expect()`
- Prefer `anyhow::Result<T>` for application errors, `thiserror` for domain errors

#### Project Structure

The backend follows a layered architecture:

| Layer | Purpose | Examples |
|-------|---------|----------|
| `api/` | HTTP handlers and routing | `auth.rs`, `games.rs`, `wallet.rs` |
| `services/` | Business logic | `auth.rs`, `games.rs`, `wallet.rs` |
| `db/` | Database connections and queries | `pool.rs` |
| `middleware/` | Cross-cutting concerns | `cors.rs`, `rate_limit.rs`, `logging.rs` |

#### Error Handling

```rust
// Use thiserror for domain-specific errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("User not found")]
    UserNotFound,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

// Wrap in a Result type
pub type Result<T> = std::result::Result<T, AppError>;
```

#### Database Queries

- Use `sqlx` with compile-time query verification
- Always use parameterized queries (never string concatenation)
- Keep transactions short and explicit

```rust
// Good
let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

// Bad - SQL injection risk
let user = sqlx::query_as(&format!("SELECT * FROM users WHERE id = '{}'", user_id))
```

### Frontend (React) Conventions

#### Code Style

- **Lint code**:
  ```bash
  npm run lint
  ```
- **Build for production**:
  ```bash
  npm run build
  ```

#### Component Structure

Use functional components with hooks:

```jsx
import { useState, useEffect } from 'react';
import { useAuth } from '../hooks/useAuth';

export function ComponentName() {
  const { user, login, logout } = useAuth();
  const [state, setState] = useState(initialValue);

  useEffect(() => {
    // effect logic
    return () => {
      // cleanup logic
    };
  }, [dependency]);

  return (
    <div>
      {/* JSX */}
    </div>
  );
}
```

#### Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Components | PascalCase | `GameCard.jsx` |
| Hooks | camelCase with `use` prefix | `useAuth.js` |
| Utilities | camelCase | `formatCurrency.js` |
| CSS files | kebab-case | `game-card.css` |

#### State Management

- Use React Context for global state (auth, theme)
- Use local state (`useState`) for component-specific state
- Use custom hooks to encapsulate business logic

### Code Style Guidelines

#### General Principles

1. **Write self-documenting code**: Use clear variable and function names
2. **Keep functions small**: Each function should do one thing well (single responsibility)
3. **Avoid premature optimization**: Write clear code first, optimize only when needed
4. **Handle errors explicitly**: Don't silently ignore errors

#### Git Commit Message Format

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

**Types:**

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code style changes (formatting, semicolons) |
| `refactor` | Code refactoring |
| `test` | Adding or updating tests |
| `chore` | Build process or auxiliary tool changes |

**Examples:**

```bash
feat(auth): add OAuth integration with GitHub
fix(wallet): correct USDC balance calculation for pending transactions
docs(api): update endpoint documentation for /api/games
refactor(matchmaking): extract queue management to separate module
test(api): add integration tests for auth endpoints
```

---

## Testing Guide

### Unit Tests

#### Rust Backend

Run unit tests:
```bash
cd backend
cargo test
```

Run with output capture:
```bash
cargo test -- --nocapture
```

Run specific tests:
```bash
cargo test test_function_name
```

#### React Frontend

Run unit tests with Vitest:
```bash
npm run test        # Watch mode
npm run test:run    # Single run
```

### Integration Tests

#### Backend Integration Tests

Integration tests run against a real database. Configure the test database URL in your environment:

```bash
export DATABASE_URL="postgres://postgres:postgres@localhost:54322/magnetite_test"
cargo test --test integration
```

#### Frontend Integration Tests

React component tests use Testing Library:

```jsx
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Register from '../pages/Register';

describe('Register', () => {
  it('renders registration form', () => {
    render(
      <MemoryRouter>
        <Register />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /sign up/i })).toBeInTheDocument();
  });
});
```

### E2E Tests

E2E tests use Playwright. First, ensure the development servers are running, then:

```bash
# Install Playwright browsers (first time only)
npx playwright install

# Run E2E tests
npx playwright test
```

#### E2E Test Structure

```
e2e/
├── page-objects/       # Page object models
│   ├── base.page.js
│   ├── login.page.js
│   ├── marketplace.page.js
│   └── navigation.page.js
└── tests/              # Test specs (add your tests here)
```

#### Writing E2E Tests

```javascript
import { test, expect } from '@playwright/test';
import { LoginPage } from '../page-objects/login.page';

test.describe('Authentication', () => {
  let loginPage;

  test.beforeEach(async ({ page }) => {
    loginPage = new LoginPage(page);
    await loginPage.goto();
  });

  test('user can login with valid credentials', async () => {
    await loginPage.fillUsername('testuser');
    await loginPage.fillPassword('password123');
    await loginPage.clickLogin();
    await expect(page).toHaveURL('/dashboard');
  });
});
```

### Coverage Requirements

| Layer | Minimum Coverage | Tool |
|-------|------------------|------|
| Backend | 80% | `cargo tarpaulin` |
| Frontend | 70% | Vitest coverage |

Run coverage reports:

```bash
# Backend
cargo install cargo-tarpaulin
cargo tarpaulin --out Html

# Frontend
npm run test:run -- --coverage
```

---

## Pull Request Process

### Branch Naming

Use the following conventions:

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feature/<short-description>` | `feature/oauth-github` |
| Bug Fix | `fix/<short-description>` | `fix/wallet-balance` |
| Hotfix | `hotfix/<short-description>` | `hotfix/payment-deadlock` |
| Refactor | `refactor/<short-description>` | `refactor/auth-module` |
| Docs | `docs/<short-description>` | `docs/api-documentation` |

### PR Template

When opening a PR, use this template:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix (non-breaking change)
- [ ] New feature (non-breaking change)
- [ ] Breaking change (requires PR description)
- [ ] Documentation update
- [ ] Refactoring

## Testing
How was this tested?

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] E2E tests added/updated
- [ ] Manual testing performed

## Checklist
- [ ] My code follows the style guidelines
- [ ] I have performed self-review
- [ ] I have commented complex code
- [ ] I have updated documentation
- [ ] My changes generate no new warnings
- [ ] Tests pass locally
- [ ] Dependencies updated (if applicable)
```

### Review Requirements

1. **Self-review**: Review your own code before requesting reviews
2. **At least 1 approval**: One maintainer must approve before merge
3. **All checks pass**: CI must pass on all checks
4. **No unresolved conversations**: All review comments must be resolved

### Merge Criteria

- [ ] Code follows project conventions
- [ ] Tests pass (unit, integration, E2E)
- [ ] Lint and type checks pass
- [ ] Documentation updated (if applicable)
- [ ] CHANGELOG updated (for user-facing changes)
- [ ] No merge conflicts

### Merge Strategy

We use **squash merge** for feature branches. This keeps the main branch history clean while preserving the PR description as the commit message.

---

## Release Process

### Versioning

We follow [Semantic Versioning](https://semver.org/):

```
MAJOR.MINOR.PATCH
  │     │     └─ Patch: Bug fixes, no API changes
  │     └─ Minor: New features, backwards compatible
  └─ Major: Breaking changes
```

Current version is defined in:
- `backend/Cargo.toml` → `version = "0.1.0"`
- `package.json` → `"version": "0.0.0"`

### Changelog Updates

The changelog is maintained in `CHANGELOG.md`. For each release, add:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New features

### Changed
- Changes in existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security improvements
```

### Release Checklist

1. **Update version numbers** in `Cargo.toml` and `package.json`
2. **Update CHANGELOG.md** with all changes since last release
3. **Create GitHub tag**:
   ```bash
   git tag -a v0.1.0 -m "Release version 0.1.0"
   git push origin v0.1.0
   ```
4. **Create GitHub Release** with the changelog entries
5. **Verify CI passes** on the release tag
6. **Monitor deployment** on Fly.io

### Release Branches

For major releases, create a release branch:

```bash
git checkout -b release/1.0.0
# Make release-specific changes
git push origin release/1.0.0
```

---

## Community

### Discord

Join our Discord server for real-time discussions:
- **Link**: https://discord.gg/magnetite (placeholder - update with actual link)

### GitHub Discussions

For questions, ideas, and longer discussions, use GitHub Discussions:
- **Link**: https://github.com/anomalyco/magnetite/discussions

### Bug Reports

Before submitting a bug report:

1. **Search existing issues** to avoid duplicates
2. **Use the bug report template** when creating an issue
3. **Include**:
   - Clear description of the issue
   - Steps to reproduce
   - Expected vs actual behavior
   - Environment details (OS, Rust version, Node version)
   - Relevant logs or screenshots

### Feature Requests

We welcome feature requests! Before submitting:

1. **Search existing issues** to see if it's already requested
2. **Consider the scope**: Is this within the project's goals?
3. **Use the feature request template** when creating an issue
4. **Explain the use case**: Why would this feature be valuable?

### Good First Issues

Looking to contribute but not sure where to start? Check out issues labeled:
- `good first issue` - Entry-level tasks for new contributors
- `help wanted` - Issues where we'd appreciate contributions
- `documentation` - Documentation improvements

---

## License

By contributing to Magnetite, you agree that your contributions will be licensed under the MIT License. See [LICENSE](LICENSE) for details.
