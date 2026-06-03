import { lazy, Suspense, useState, useCallback } from 'react';
import { BrowserRouter, Routes, Route, useNavigate } from 'react-router-dom';
import { ToastProvider, useToast } from './context/ToastContext';
import { AnnouncementProvider } from './context/AnnouncementContext';
import { ThemeProvider } from './context/ThemeContext';
import { AccessibilityProvider } from './components/AccessibilityProvider';
import { CommsProvider } from './context/CommsContext';
import { NotificationProvider } from './context/NotificationContext';
import { I18nProvider } from './i18n/I18nProvider';
import AnnouncementBanner from './components/AnnouncementBanner';
import Toast from './components/Toast';
import PageLoader from './components/PageLoader';
import ErrorBoundary from './components/ErrorBoundary';
import { AdminRoute } from './components/admin/AdminRoute';
import { SearchModal } from './components';
import KeyboardShortcuts from './components/KeyboardShortcuts';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import './index.css';
import './assets/skip-link.css';

const LandingPage = lazy(() => import('./components/landing/LandingPage'));
const Marketplace = lazy(() => import('./pages/Marketplace'));
const GameDetail = lazy(() => import('./pages/GameDetail'));
const Login = lazy(() => import('./pages/Login'));
const Register = lazy(() => import('./pages/Register'));
const ForgotPassword = lazy(() => import('./pages/ForgotPassword'));
const ResetPassword = lazy(() => import('./pages/ResetPassword'));
const VerifyEmail = lazy(() => import('./pages/VerifyEmail'));
const AuthCallback = lazy(() => import('./pages/AuthCallback'));
const LinkAccount = lazy(() => import('./pages/LinkAccount'));
const ConnectedAccounts = lazy(() => import('./pages/ConnectedAccounts'));
const DeveloperDashboard = lazy(() => import('./pages/DeveloperDashboard'));
const GameDeploy = lazy(() => import('./pages/developers/GameDeploy'));
const GameAnalytics = lazy(() => import('./pages/GameAnalytics'));
const Wallet = lazy(() => import('./pages/Wallet'));
const GameStudio = lazy(() => import('./pages/GameStudio'));
const Earnings = lazy(() => import('./pages/Earnings'));
const Settings = lazy(() => import('./pages/Settings'));
const Security = lazy(() => import('./pages/Security'));
const PrivacySettings = lazy(() => import('./pages/PrivacySettings'));
const Privacy = lazy(() => import('./pages/Privacy'));
const Terms = lazy(() => import('./pages/Terms'));
const Cookies = lazy(() => import('./pages/Cookies'));
const About = lazy(() => import('./pages/About'));
const Contact = lazy(() => import('./pages/Contact'));
const Careers = lazy(() => import('./pages/Careers'));
const FAQ = lazy(() => import('./pages/FAQ'));
const Playground = lazy(() => import('./pages/Playground'));
const Subscription = lazy(() => import('./pages/Subscription'));
const GameAccess = lazy(() => import('./pages/GameAccess'));
const GameLobby = lazy(() => import('./pages/GameLobby'));
const Spectator = lazy(() => import('./pages/Spectator'));
const Friends = lazy(() => import('./pages/Friends'));
const Leaderboard = lazy(() => import('./pages/Leaderboard'));
const Achievements = lazy(() => import('./pages/Achievements'));
const Profile = lazy(() => import('./pages/Profile'));
const EditProfile = lazy(() => import('./pages/EditProfile'));
const Wishlist = lazy(() => import('./pages/Wishlist'));
const Onboarding = lazy(() => import('./pages/Onboarding'));
const Welcome = lazy(() => import('./pages/Welcome'));
const Pricing = lazy(() => import('./pages/Pricing'));
const Forbidden = lazy(() => import('./pages/Forbidden'));
const NotFound = lazy(() => import('./pages/NotFound'));
const ServerError = lazy(() => import('./pages/ServerError'));
const AdminDashboard = lazy(() => import('./pages/admin/AdminDashboard'));
const AdminUsers = lazy(() => import('./pages/admin/Users'));
const AdminGames = lazy(() => import('./pages/admin/Games'));
const AdminFinance = lazy(() => import('./pages/admin/Finance'));
const AdminSettings = lazy(() => import('./pages/admin/Settings'));
const AdminReviewModeration = lazy(() => import('./pages/admin/ReviewModeration'));
const AdminModeration = lazy(() => import('./pages/admin/Moderation'));
const Communities = lazy(() => import('./pages/Communities'));
const Messages = lazy(() => import('./pages/Messages'));
const Streams = lazy(() => import('./pages/Streams'));
const Points = lazy(() => import('./pages/Points'));
const DevMarketplace = lazy(() => import('./pages/DevMarketplace'));
const ControllerSettings = lazy(() => import('./pages/ControllerSettings'));

function ToastContainer() {
  const { toasts, removeToast } = useToast();
  return (
    <div className="toast-container">
      {toasts.map(toast => (
        <Toast key={toast.id} toast={toast} onRemove={removeToast} />
      ))}
    </div>
  );
}

function AppContent() {
  const navigate = useNavigate();
  const [showSearch, setShowSearch] = useState(false);
  const [showShortcuts, setShowShortcuts] = useState(false);

  const handleCloseModal = useCallback(() => {
    setShowSearch(false);
    setShowShortcuts(false);
  }, []);

  const shortcuts = [
    { key: '?', description: 'Show shortcuts', action: () => setShowShortcuts(true), allowInInput: true },
    { key: 'k', description: 'Open search', action: () => setShowSearch(true), allowInInput: true },
    { key: 'g h', description: 'Go to home', action: () => navigate('/home') },
    { key: 'g m', description: 'Go to marketplace', action: () => navigate('/marketplace') },
    { key: 'g d', description: 'Go to developer dashboard', action: () => navigate('/developers') },
    { key: 'c', description: 'Open wallet', action: () => navigate('/wallet') },
    { key: 'Esc', description: 'Close modal', action: handleCloseModal, allowInInput: true },
  ];

  useKeyboardShortcuts(shortcuts);

  return (
    <>
      <SearchModal isOpen={showSearch} onClose={() => setShowSearch(false)} />
      <KeyboardShortcuts isOpen={showShortcuts} onClose={() => setShowShortcuts(false)} />
    </>
  );
}

function App() {
  return (
    <I18nProvider>
    <AccessibilityProvider>
      <ThemeProvider>
        <AnnouncementProvider announcement="Magnetite v2.0 launching soon! New features and improvements coming your way.">
          <ToastProvider>
            <NotificationProvider>
            <CommsProvider>
              <a href="#main-content" className="skip-link">
                Skip to main content
              </a>
              <AnnouncementBanner />
              <ToastContainer />
              <ErrorBoundary>
                <BrowserRouter>
                  <AppContent />
                  <Suspense fallback={<PageLoader />}>
                    <Routes>
                      <Route path="/" element={<Marketplace />} />
                      <Route path="/home" element={<LandingPage />} />
                      <Route path="/login" element={<Login />} />
                      <Route path="/register" element={<Register />} />
                      <Route path="/forgot-password" element={<ForgotPassword />} />
                      <Route path="/reset-password" element={<ResetPassword />} />
                      <Route path="/verify-email" element={<VerifyEmail />} />
                      <Route path="/auth/callback" element={<AuthCallback />} />
                      <Route path="/settings/linked-accounts" element={<LinkAccount />} />
                      <Route path="/settings/connected-accounts" element={<ConnectedAccounts />} />
                      <Route path="/marketplace" element={<Marketplace />} />
                      <Route path="/game/:id" element={<GameDetail />} />
                      <Route path="/play/:id" element={<Playground />} />
                      <Route path="/subscription" element={<Subscription />} />
                      <Route path="/game-access" element={<GameAccess />} />
                      <Route path="/lobby/:id" element={<GameLobby />} />
                      <Route path="/spectate/:id" element={<Spectator />} />
                      <Route path="/developers" element={<DeveloperDashboard />} />
                      <Route path="/developers/studio" element={<GameStudio />} />
                      <Route path="/developers/deploy" element={<GameDeploy />} />
                      <Route path="/developers/earnings" element={<Earnings />} />
                      <Route path="/developers/analytics/:gameId" element={<GameAnalytics />} />
                      <Route path="/developers/settings" element={<Settings />} />
                      <Route path="/wallet" element={<Wallet />} />
                      <Route path="/friends" element={<Friends />} />
                      <Route path="/leaderboard" element={<Leaderboard />} />
                      <Route path="/achievements" element={<Achievements />} />
                      <Route path="/profile/:username" element={<Profile />} />
                      <Route path="/edit-profile" element={<EditProfile />} />
                      <Route path="/wishlist" element={<Wishlist />} />
                      <Route path="/onboarding" element={<Onboarding />} />
                      <Route path="/welcome" element={<Welcome />} />
                      <Route path="/pricing" element={<Pricing />} />
                      <Route path="/about" element={<About />} />
                      <Route path="/contact" element={<Contact />} />
                      <Route path="/careers" element={<Careers />} />
                      <Route path="/faq" element={<FAQ />} />
                      <Route path="/settings" element={<Settings />} />
                      <Route path="/settings/security" element={<Security />} />
                      <Route path="/settings/privacy" element={<PrivacySettings />} />
                      <Route path="/privacy" element={<Privacy />} />
                      <Route path="/terms" element={<Terms />} />
                      <Route path="/cookies" element={<Cookies />} />
                      <Route path="/403" element={<Forbidden />} />
                      <Route path="/500" element={<ServerError />} />
                      <Route path="/admin" element={<AdminRoute><AdminDashboard /></AdminRoute>} />
                      <Route path="/admin/users" element={<AdminRoute><AdminUsers /></AdminRoute>} />
                      <Route path="/admin/games" element={<AdminRoute><AdminGames /></AdminRoute>} />
                      <Route path="/admin/finance" element={<AdminRoute><AdminFinance /></AdminRoute>} />
                      <Route path="/admin/settings" element={<AdminRoute><AdminSettings /></AdminRoute>} />
                      <Route path="/admin/review-moderation" element={<AdminRoute><AdminReviewModeration /></AdminRoute>} />
                      <Route path="/admin/moderation" element={<AdminRoute><AdminModeration /></AdminRoute>} />
                      <Route path="/communities" element={<Communities />} />
                      <Route path="/messages" element={<Messages />} />
                      <Route path="/streams" element={<Streams />} />
                      <Route path="/points" element={<Points />} />
                      <Route path="/developers/marketplace" element={<DevMarketplace />} />
                      <Route path="/settings/controller" element={<ControllerSettings />} />
                      <Route path="*" element={<NotFound />} />
                    </Routes>
                  </Suspense>
                </BrowserRouter>
              </ErrorBoundary>
            </CommsProvider>
            </NotificationProvider>
          </ToastProvider>
        </AnnouncementProvider>
      </ThemeProvider>
    </AccessibilityProvider>
    </I18nProvider>
  );
}

export default App;
