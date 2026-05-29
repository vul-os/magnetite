# Magnetite - Implementation Tasks

## Phase 1: Foundation

### Platform API
- [x] Auth endpoints (register, login, me)
- [x] Wallet endpoints (balance, deposit, withdraw)
- [x] Games CRUD endpoints
- [x] Matchmaking endpoints
- [x] Health check endpoint

### OAuth Integration
- [ ] Google OAuth
- [ ] Discord OAuth
- [ ] GitHub OAuth
- [ ] GitLab OAuth
- [ ] OAuth callback handling
- [ ] Link/unlink accounts

### Frontend - Auth Pages
- [ ] Login page with email/password
- [ ] Login page with OAuth buttons
- [ ] Registration page
- [ ] Forgot password page
- [ ] Reset password page
- [ ] Email verification page
- [ ] Auth callback page

### Frontend - Core Pages
- [x] Marketplace listing page
- [x] Game detail page
- [x] Developer dashboard
- [x] Wallet page
- [x] Matchmaking page
- [x] Home/landing page

### Frontend - Developer Portal
- [x] Developer dashboard overview
- [x] Game studio page
- [x] Earnings page
- [x] Settings page
- [ ] Analytics charts
- [ ] Game deployment flow

## Phase 2: Rust Integration

### SDK
- [ ] magnetite-sdk Rust crate
- [ ] GameLogic trait
- [ ] Input/Output types
- [ ] State sync protocol

### Game Engine
- [ ] Bevy game template
- [ ] WASM build pipeline
- [ ] Client/server communication

### Real-time
- [x] WebSocket game connections
- [ ] Game state broadcasting
- [ ] Player input handling
- [ ] Latency compensation

## Phase 3: Infrastructure

### Database
- [x] PostgreSQL schema
- [x] Migration system (reset, up, status)
- [ ] Redis for sessions/cache
- [ ] Database backups

### Email
- [ ] Email provider integration (SES/Resend/SMTP)
- [ ] Email templates:
  - [ ] Welcome email
  - [ ] Email verification
  - [ ] Password reset
  - [ ] Game invite
  - [ ] Payout notifications
  - [ ] Anti-cheat alerts

### GitHub Integration
- [ ] GitHub App setup
- [ ] Webhook handler
- [ ] CI/CD pipeline
- [ ] Repo verification
- [ ] Auto-deploy on push

### Deployment
- [ ] Docker setup
- [ ] Fly.io deployment
- [ ] Self-hosting guide
- [ ] Docker compose for local dev

## Phase 4: Security

### Authentication
- [x] JWT tokens
- [x] Password hashing (argon2)
- [x] Session management
- [ ] Refresh tokens
- [ ] Token rotation
- [ ] Force logout

### Anti-Cheat
- [ ] Velocity detection
- [ ] Anomaly detection
- [ ] Device fingerprinting
- [ ] Global ban list
- [ ] Replay storage
- [ ] Server-authoritative state

### Game Isolation
- [ ] Wasmtime sandbox
- [ ] gVisor container
- [ ] Resource limits
- [ ] Timeout handling

## Phase 5: Payments

### USDC Integration
- [ ] Circle SDK integration
- [ ] Wallet creation
- [ ] Deposit flow
- [ ] Withdrawal flow
- [ ] Balance management

### Fiat On-Ramp
- [ ] Paystack integration (SA)
- [ ] ZAR to USDC conversion
- [ ] Payment webhook handling

### Developer Payouts
- [ ] Earnings calculation (70/30 split)
- [ ] Minimum payout enforcement
- [ ] Weekly auto-payout
- [ ] On-demand payout
- [ ] Payout history

## Phase 6: Features

### Matchmaking
- [x] Queue entry
- [x] Queue status
- [x] Leave queue
- [ ] Skill-based matching
- [ ] Region-based matching
- [ ] Party matching

### Leaderboards
- [ ] Global leaderboards
- [ ] Friend leaderboards
- [ ] Weekly/monthly resets
- [ ] Score submission
- [ ] Rank display

### Achievements
- [ ] Achievement definitions
- [ ] Progress tracking
- [ ] Unlock notifications
- [ ] Achievement gallery

### Social
- [ ] Friend list
- [ ] Game invites
- [ ] Spectator mode
- [ ] Chat (in-game)

## Phase 7: Documentation

### User Docs
- [ ] Getting started guide
- [ ] Account creation guide
- [ ] Wallet setup guide
- [ ] Playing games guide
- [ ] Developer quickstart

### Developer Docs
- [ ] SDK documentation
- [ ] Game submission guide
- [ ] API reference
- [ ] CI/CD setup
- [ ] Webhook reference

### Platform Docs
- [ ] Architecture overview
- [ ] Self-hosting guide
- [ ] Docker deployment
- [ ] Database schema
- [ ] Security practices

## Phase 8: Polish

### UI/UX
- [ ] Landing page hero
- [ ] Sign in/sign up illustrations
- [ ] Forgot password flow illustration
- [ ] Empty states
- [ ] Loading states
- [ ] Error pages
- [ ] Responsive design

### Branding
- [ ] Logo variants
- [ ] Color palette
- [ ] Typography
- [ ] Icon set
- [ ] Mascot/illustrations

### Performance
- [ ] Frontend bundle optimization
- [ ] API response caching
- [ ] Database query optimization
- [ ] CDN setup
- [ ] Image optimization

## Frontend Tasks (Detailed)

### Pages to Create/Update
- [ ] src/pages/Home.jsx - Landing page with hero
- [ ] src/pages/Login.jsx - Email + OAuth login
- [ ] src/pages/Register.jsx - Email + OAuth registration
- [ ] src/pages/ForgotPassword.jsx - Password reset request
- [ ] src/pages/ResetPassword.jsx - Set new password
- [ ] src/pages/VerifyEmail.jsx - Email verification
- [ ] src/pages/Marketplace.jsx - Game listing
- [ ] src/pages/GameDetail.jsx - Single game page
- [ ] src/pages/DeveloperDashboard.jsx - Dev overview
- [ ] src/pages/GameStudio.jsx - Game creation/editing
- [ ] src/pages/Earnings.jsx - Developer earnings
- [ ] src/pages/Settings.jsx - User settings
- [ ] src/pages/Wallet.jsx - Wallet management
- [ ] src/pages/Matchmaking.jsx - Find players
- [ ] src/pages/AuthCallback.jsx - OAuth callback
- [ ] src/pages/LinkedAccounts.jsx - Connected OAuth
- [ ] src/pages/Leaderboard.jsx - Game leaderboard
- [ ] src/pages/Achievements.jsx - User achievements
- [ ] src/pages/Friends.jsx - Social features

### Components to Create
- [ ] src/components/Hero.jsx - Landing hero section
- [ ] src/components/Features.jsx - Platform features
- [ ] src/components/Pricing.jsx - Pricing explanation
- [ ] src/components/Testimonials.jsx - User testimonials
- [ ] src/components/FAQ.jsx - Common questions
- [ ] src/components/Footer.jsx - Site footer
- [ ] src/components/Navbar.jsx - Navigation
- [ ] src/components/GameCard.jsx - Game preview card
- [ ] src/components/GameGrid.jsx - Multiple games
- [ ] src/components/LeaderboardTable.jsx - Score table
- [ ] src/components/SessionCard.jsx - Play session
- [ ] src/components/TransactionList.jsx - Wallet history
- [ ] src/components/StatsCard.jsx - Dashboard stat
- [ ] src/components/Chart.jsx - Analytics chart
- [ ] src/components/Modal.jsx - Reusable modal
- [ ] src/components/Button.jsx - Styled button
- [ ] src/components/Input.jsx - Styled input
- [ ] src/components/Select.jsx - Styled select
- [ ] src/components/Spinner.jsx - Loading spinner
- [ ] src/components/Toast.jsx - Notifications

### Hooks to Create
- [ ] src/hooks/useAuth.js - Authentication state
- [ ] src/hooks/useWallet.js - Wallet operations
- [ ] src/hooks/useGames.js - Game data fetching
- [ ] src/hooks/useMatchmaking.js - Queue management
- [ ] src/hooks/useToast.js - Notifications
- [ ] src/hooks/useLocalStorage.js - Persistent state
- [ ] src/hooks/useWebSocket.js - WS connection

### Context Providers
- [x] src/context/AuthContext.jsx
- [x] src/context/WalletContext.jsx
- [x] src/context/GameContext.jsx
- [ ] src/context/SocketContext.jsx - WebSocket state
- [ ] src/context/ToastContext.jsx - Notifications

## Backend Tasks (Detailed)

### API Modules
- [x] src/api/auth.rs - User authentication
- [x] src/api/wallet.rs - Wallet operations
- [x] src/api/games.rs - Game CRUD
- [x] src/api/matchmaking.rs - Queue management
- [ ] src/api/oauth.rs - OAuth flows
- [ ] src/api/developer.rs - Developer tools
- [ ] src/api/admin.rs - Admin panel
- [ ] src/api/leaderboard.rs - Score management
- [ ] src/api/achievements.rs - Achievement tracking
- [ ] src/api/social.rs - Friends/invites

### Services
- [x] src/services/auth.rs - Auth business logic
- [x] src/services/wallet.rs - Wallet logic
- [x] src/services/games.rs - Game logic
- [ ] src/services/matchmaking.rs - Match logic
- [ ] src/services/email.rs - Email sending
- [ ] src/services/payment.rs - Payment processing
- [ ] src/services/anticheat.rs - Anti-cheat logic

### Middleware
- [x] src/middleware.rs - Auth middleware

### WebSocket
- [x] src/ws/game.rs - Game WS handler

## Database Tasks

### Tables
- [x] users
- [x] games
- [x] wallet_balances
- [x] wallet_transactions
- [x] transactions
- [x] scores
- [x] game_high_scores
- [x] play_sessions
- [x] matchmaking_queue
- [x] sessions
- [x] github_installations
- [x] registered_games
- [x] payouts
- [x] admin_actions
- [x] _migrations

### Indexes
- [x] All basic indexes
- [ ] Partial indexes for active records
- [ ] Composite indexes for common queries

## DevOps Tasks

### Docker
- [ ] Dockerfile for backend
- [ ] Dockerfile for frontend
- [ ] docker-compose.yml
- [ ] nginx configuration

### CI/CD
- [ ] GitHub Actions workflow
- [ ] Test automation
- [ ] Build and deploy
- [ ] Database migrations

### Monitoring
- [ ] Logging setup
- [ ] Error tracking
- [ ] Performance metrics
- [ ] Uptime monitoring

## Testing Tasks

### Backend Tests
- [ ] Unit tests for services
- [ ] Integration tests for API
- [ ] WebSocket tests
- [ ] Auth flow tests

### Frontend Tests
- [ ] Component tests
- [ ] Page tests
- [ ] Hook tests
- [ ] E2E tests

### QA
- [ ] Playwright setup
- [ ] Test fixtures
- [ ] CI integration
