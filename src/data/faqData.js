export const faqData = [
  {
    category: "Getting Started",
    icon: "🎮",
    questions: [
      {
        q: "How do I start playing?",
        a: "Create an account by clicking the Sign Up button, verify your email address, and browse our game marketplace. Free games need nothing else. If you want to buy an item or a paid session, link a wallet you control and pay in USDC — Magnetite never holds a balance for you."
      },
      {
        q: "How do I create an account?",
        a: "Visit our registration page and enter your email address, username, and password. You'll receive a verification email - click the link inside to activate your account. You can also sign up using Google or Discord for faster access."
      },
      {
        q: "How do I link my wallet?",
        a: "Go to your Wallet page and paste the 32-byte hex Ed25519 public key of a wallet you control. There is nothing to top up: your wallet stays yours, and each purchase pays the developer or operator directly from it."
      },
      {
        q: "Is there a minimum amount I have to load up front?",
        a: "No. Because Magnetite is non-custodial there is no deposit and no stored balance — you pay per purchase, straight from your own wallet, for exactly the amount shown at checkout."
      },
      {
        q: "Is my information secure?",
        a: "Yes. All traffic is TLS-encrypted, and because payments settle wallet to wallet we never hold your funds or store card details at all. Your private key never leaves your wallet — Magnetite only ever sees your public key."
      }
    ]
  },
  {
    category: "For Players",
    icon: "🏆",
    questions: [
      {
        q: "How do matches work?",
        a: "Our matchmaking system automatically pairs you with players of similar skill levels. When you click Play on any game, you'll enter a queue. Once matched, you'll have a set time to complete the session, and winners are determined by the game's rules."
      },
      {
        q: "What payment methods do you accept?",
        a: "Payments settle in USDC from a wallet you control. Checkout is a single atomic transaction that pays the developer (and the server operator, for hosting fees) and mints a signed receipt — there is no intermediary holding the money."
      },
      {
        q: "How do I withdraw my earnings?",
        a: "There is nothing to withdraw. Earnings arrive in your own wallet at the moment of sale, so the funds are already yours — the Wallet page just lists the signed receipts that prove each settlement."
      },
      {
        q: "Why is my game not loading?",
        a: "First, check your internet connection and ensure you're using a supported browser (Chrome, Firefox, Safari, or Edge). Try clearing your browser cache and disabling any ad blockers. If the issue persists, try a different device or contact support with your game session ID."
      },
      {
        q: "How do I report a player for cheating?",
        a: "After completing a game session, you can submit a report from your match history. Include any relevant details about the suspicious behavior. Our trust and safety team reviews all reports within 24 hours and takes appropriate action."
      },
      {
        q: "Can I play with my friends?",
        a: "Yes! You can create a private room and invite friends using a shareable link. Private matches don't affect rankings but still award XP and achievements."
      }
    ]
  },
  {
    category: "For Developers",
    icon: "⚙️",
    questions: [
      {
        q: "How do I publish my game?",
        a: "First, apply for a developer account through our Game Studio page. Once approved, connect your GitHub repository, configure your game's settings, and submit for review. Our team typically reviews submissions within 5-7 business days."
      },
      {
        q: "What are the revenue share terms?",
        a: "The buyer's wallet pays your wallet the full subtotal. The protocol fee defaults to 0 bps, so by default you keep 100% of the sale, and there are no upfront costs to publish. Where a hosting fee applies it goes to the server operator, split atomically in the same transaction."
      },
      {
        q: "Do you provide an SDK?",
        a: "Yes, we offer a Rust SDK built on Bevy. Our SDK handles matchmaking, secure payment processing, anti-cheat integration, and player analytics out of the box. Compile to WASM for browsers or native for desktop."
      },
      {
        q: "How do payouts work?",
        a: "There are no payouts, because there is no float. Each purchase settles straight into your wallet and mints a signed receipt; the only thing you configure is which wallet address to be paid at."
      },
      {
        q: "What are the technical requirements for games?",
        a: "Games must be written in Rust using Bevy and compile to WASM or native via the Magnetite SDK. Maximum session duration is 60 minutes. Games must include our anti-cheat module and support our authentication flow. See our developer documentation for full specifications."
      },
      {
        q: "Can I use my own payment system?",
        a: "Entitlements are granted from signed receipts, so anything sold in-platform has to go through the payment rail that mints them. That rail is non-custodial and its protocol fee defaults to 0 bps, so it costs you nothing to use — and you keep your own wallet either way."
      }
    ]
  },
  {
    category: "Security",
    icon: "🔒",
    questions: [
      {
        q: "Is my money safe?",
        a: "We never take custody of it, which is the strongest guarantee we can offer: there is no platform balance to freeze, lose or misuse. Funds sit in your wallet until you spend them, and every settlement leaves a signed receipt you can verify yourself."
      },
      {
        q: "How does anti-cheat work?",
        a: "Our multi-layered anti-cheat system includes server-side validation, behavioral analysis, and machine learning detection. Games must integrate our anti-cheat SDK, which monitors for common cheating methods like aimbots and speed hacks."
      },
      {
        q: "What happens if I get accused of cheating falsely?",
        a: "If you believe your account was banned incorrectly, you can submit an appeal through our support portal. Our trust and safety team will review your case and game logs. False positives are rare but we do overturn bans when evidence supports it."
      },
      {
        q: "How do you protect against match manipulation?",
        a: "Match results are server-authoritative and every session produces a replay log that anyone can re-simulate to detect tampering. Suspicious patterns and colluding players are flagged for manual review before any action is taken."
      },
      {
        q: "Is my personal data protected?",
        a: "Yes. We comply with GDPR and CCPA regulations. You can request a copy of your data or request deletion at any time through your account settings. We never sell personal information to third parties."
      }
    ]
  },
  {
    category: "Technical",
    icon: "💻",
    questions: [
      {
        q: "What devices are supported?",
        a: "Magnetite works on any modern web browser including Chrome, Firefox, Safari, and Edge on Windows, macOS, and Linux. We also have native apps for iOS and Android. Console support is coming soon."
      },
      {
        q: "What are the system requirements?",
        a: "For web: A device with HTML5 support and a stable internet connection (minimum 5 Mbps). For mobile: iOS 14+ or Android 10+. Our games are optimized to run smoothly on mid-range devices."
      },
      {
        q: "Why am I experiencing lag?",
        a: "Lag can be caused by several factors: slow internet connection, high latency to our servers, or high resource usage on your device. Try closing other browser tabs, using a wired connection instead of WiFi, or selecting a server region closer to your location."
      },
      {
        q: "Do I need to download anything?",
        a: "No downloads required for web games - everything runs directly in your browser. Our optional desktop app provides a more optimized experience with lower latency and built-in voice chat, but it's not required."
      },
      {
        q: "How do I update the desktop app?",
        a: "The desktop app updates automatically in the background. You'll receive a notification when an update is ready to install. You can also manually check for updates in Settings > About."
      }
    ]
  }
];

export const contactInfo = {
  email: "support@magnetite.gg",
  discord: "discord.gg/magnetite",
  twitter: "@MagnetiteGG"
};
