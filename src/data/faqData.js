export const faqData = [
  {
    category: "Getting Started",
    icon: "🎮",
    questions: [
      {
        q: "How do I start playing?",
        a: "Create an account by clicking the Sign Up button, verify your email address, add funds to your wallet using a credit card or bank transfer via Paystack, and browse our game marketplace to find matches. Once you find a game you like, click Play and you'll be matched with other players."
      },
      {
        q: "How do I create an account?",
        a: "Visit our registration page and enter your email address, username, and password. You'll receive a verification email - click the link inside to activate your account. You can also sign up using Google or Discord for faster access."
      },
      {
        q: "How do I add funds to my wallet?",
        a: "Navigate to your Wallet page and click Add Funds. We accept major credit cards (Visa, Mastercard, American Express) and bank transfers via Paystack. Funds are typically available instantly after payment confirmation."
      },
      {
        q: "What is the minimum deposit amount?",
        a: "The minimum deposit is $5.00 USD. There's no maximum deposit limit, but transactions over $10,000 may require additional verification."
      },
      {
        q: "Is my information secure?",
        a: "Yes. We use bank-level 256-bit SSL encryption for all data transmission. Your payment information is processed by PCI-compliant payment providers and we never store your full credit card details on our servers."
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
        a: "We accept credit and debit cards (Visa, Mastercard, Amex) and bank transfers via Paystack. All transactions are secured with end-to-end encryption. No cryptocurrency required."
      },
      {
        q: "How do I withdraw my earnings?",
        a: "Go to your Wallet page and click Withdraw. Enter the amount and your bank details. Withdrawals are processed via Wise and typically arrive in your bank account within 1-2 business days."
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
        a: "Developers receive 70% of all session fees collected from their games. There are no upfront costs to publish — we only earn when you earn. Payouts are processed weekly via Wise to your connected bank account for balances over $50."
      },
      {
        q: "Do you provide an SDK?",
        a: "Yes, we offer a Rust SDK built on Bevy. Our SDK handles matchmaking, secure payment processing, anti-cheat integration, and player analytics out of the box. Compile to WASM for browsers or native for desktop."
      },
      {
        q: "How do payouts work?",
        a: "Earnings are calculated weekly. If your balance exceeds $50, we process a payout to your connected bank account via Wise. Transfers typically arrive within 1-2 business days. You configure your bank details once in Developer Settings."
      },
      {
        q: "What are the technical requirements for games?",
        a: "Games must be written in Rust using Bevy and compile to WASM or native via the Magnetite SDK. Maximum session duration is 60 minutes. Games must include our anti-cheat module and support our authentication flow. See our developer documentation for full specifications."
      },
      {
        q: "Can I use my own payment system?",
        a: "No, all payments must go through our platform to ensure secure escrow and fair matchmaking. This protects both players and developers and enables our anti-cheat system to function properly."
      }
    ]
  },
  {
    category: "Security",
    icon: "🔒",
    questions: [
      {
        q: "Is my money safe?",
        a: "Absolutely. Player funds are held in a segregated escrow account and never used for company operations. All transactions use 256-bit SSL encryption. We're also SOC 2 Type II compliant and regular third-party security audits are conducted."
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
        a: "Our system detects unusual betting patterns and match outcomes. We use algorithmic analysis to identify rings of colluding players. All suspicious activity triggers manual review before any action is taken."
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
