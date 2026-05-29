# Game Submission

Submit your game to the Magnetite platform for review and deployment.

## CI/CD Requirements

Your game must pass all CI checks before deployment.

### Build Pipeline

```yaml
# .magnetite/ci.yaml
stages:
  - build
  - test
  - security
  - deploy

build:
  script:
    - cargo build --release
    - cargo test
  artifacts:
    paths:
      - target/release/my_game

test:
  script:
    - cargo clippy
    - cargo fmt --check
  coverage: true
```

### Automated Checks

| Check | Tool | Required |
|-------|------|----------|
| Compilation | cargo build | Yes |
| Unit Tests | cargo test | Yes |
| Linting | cargo clippy | Yes |
| Formatting | cargo fmt | Yes |
| Security | cargo audit | Yes |
| Coverage | tarpaulin | No |

### Required Files

```
my-game/
├── Cargo.toml
├── magnetite.yaml
├── .magnetite/
│   ├── ci.yaml
│   └── icon.png
├── src/
│   └── lib.rs
└── README.md
```

## Security Scan

All games undergo automated security scanning.

### Prohibited Patterns

```rust
// ❌ Forbidden: File system access
std::fs::read("secrets.txt")

// ❌ Forbidden: Network requests
reqwest::get("https://evil.com")

// ❌ Forbidden: Environment reading
std::env::var("API_KEY")

// ❌ Forbidden: Arbitrary code execution
std::process::Command::new("rm").arg("-rf").spawn()
```

### Allowed APIs

```rust
// ✅ Allowed: Standard math
std::math::sin(x)

// ✅ Allowed: Game logic
HashMap::new()
Vec::push()

// ✅ Allowed: Logging
log::info!("Player moved")
```

### Running Security Checks

```bash
# Local security scan
magnetite security scan

# Check for vulnerabilities
cargo audit

# Verify no forbidden patterns
magnetite security check --allowed-only
```

## Review Process

### Submission Flow

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  SUBMIT  │────►│  BUILD   │────►│ SECURITY │────►│  REVIEW  │
└──────────┘     └──────────┘     └──────────┘     └────┬─────┘
                                                          │
                         ┌──────────┐     ┌──────────┐     │
                         │  LIVE    │◄────│ APPROVED │◄────┘
                         └──────────┘     └──────────┘
```

### Review Stages

| Stage | Duration | Description |
|-------|----------|-------------|
| Build | 5-10 min | Automated compilation and testing |
| Security | 10-15 min | Automated vulnerability scan |
| Review | 24-72 hrs | Human code review |
| Approval | - | Final review by platform team |

### Submission Command

```bash
magnetite submit --game my-game --version 1.0.0
```

### Required Metadata

```yaml
# magnetite.yaml
name: my-game
version: 1.0.0
author: your_username
description: A short description of your game
category: arcade  # arcade, puzzle, strategy, action
max_players: 4
entry_fee: 100
prize_pool: 80
```

## Versioning

### Semantic Versioning

```
major.minor.patch
1.0.0
```

| Component | Change Type | Example |
|-----------|-------------|---------|
| major | Breaking | 1.0.0 → 2.0.0 |
| minor | New feature | 1.0.0 → 1.1.0 |
| patch | Bug fix | 1.0.0 → 1.0.1 |

### Update Process

```bash
# Submit new version
magnetite update --game my-game --version 1.1.0

# Rollback if issues
magnetite rollback --game my-game --version 1.0.0
```

## Post-Deployment

### Monitoring

```bash
# View game metrics
magnetite metrics --game my-game

# Check player count
magnetite stats --game my-game --period 24h
```

### Hotfix Process

1. Fix bug in source
2. Bump patch version
3. Submit with `--hotfix` flag
4. Automatic deployment (no review)
