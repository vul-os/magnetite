import LegalLayout, { LegalSection } from '../components/LegalLayout';

const SECTIONS = [
  { id: 'what-are-cookies', title: 'What Are Cookies' },
  { id: 'how-we-use', title: 'How We Use Cookies' },
  { id: 'third-party', title: 'Third-Party Cookies' },
  { id: 'consent', title: 'Cookie Consent' },
];

export default function Cookies() {
  return (
    <LegalLayout title="Cookie Policy" lastUpdated="May 15, 2026" sections={SECTIONS}>
      <LegalSection id="what-are-cookies" title="What Are Cookies">
        <p>
          Cookies are small text files that are stored on your device (computer, tablet, or mobile) when you visit a website. They are widely used to make websites work more efficiently, provide a better user experience, and give website owners useful information.
        </p>
        <p>
          Cookies can be either "session cookies" or "persistent cookies." Session cookies are temporary and are deleted when you close your browser. Persistent cookies remain on your device for a set period or until you manually delete them.
        </p>
        <p>
          When you visit Magnetite, we may place cookies on your device to enhance your browsing experience and enable certain features.
        </p>
      </LegalSection>

      <LegalSection id="how-we-use" title="How We Use Cookies">
        <p>
          We use cookies for several purposes on Magnetite:
        </p>
        <ul>
          <li><strong>Authentication:</strong> To recognize you when you sign in and keep you logged in during your session</li>
          <li><strong>Preferences:</strong> To remember your settings and preferences, such as language and display preferences</li>
          <li><strong>Analytics:</strong> To understand how visitors use our platform, which pages are popular, and how users navigate through the site</li>
          <li><strong>Security:</strong> To detect suspicious activity and protect your account from unauthorized access</li>
          <li><strong>Performance:</strong> To monitor and improve the speed and performance of our platform</li>
          <li><strong>Game Functionality:</strong> To enable game saves, settings, and progress tracking</li>
        </ul>
        <h3>Types of Cookies We Use</h3>
        <ul>
          <li><strong>Essential Cookies:</strong> Required for the platform to function properly. They enable core features like user authentication and security.</li>
          <li><strong>Functional Cookies:</strong> Remember your preferences and settings to provide a personalized experience.</li>
          <li><strong>Analytics Cookies:</strong> Help us understand how visitors interact with our platform by collecting anonymous information.</li>
          <li><strong>Marketing Cookies:</strong> Used to deliver relevant advertisements and track campaign effectiveness (if applicable).</li>
        </ul>
      </LegalSection>

      <LegalSection id="third-party" title="Third-Party Cookies">
        <p>
          Some cookies on Magnetite are set by third-party services that appear on our platform. These third parties include:
        </p>
        <ul>
          <li><strong>Analytics Providers:</strong> Services like analytics platforms that help us understand website usage patterns</li>
          <li><strong>Game Developers:</strong> When you play third-party games, developers may set their own cookies for game functionality</li>
          <li><strong>Payment Processors:</strong> Our payment partners may set cookies to process transactions securely</li>
          <li><strong>Social Media:</strong> If you link your social media accounts or share content, those services may set cookies</li>
        </ul>
        <p>
          Third-party cookies are governed by the privacy policies of their respective services, not by this Cookie Policy. We encourage you to review the privacy policies of these third parties.
        </p>
        <h3>Managing Third-Party Cookies</h3>
        <p>
          You can manage third-party cookies through your browser settings or by visiting the third party's website directly. Disabling third-party cookies may affect the functionality of games and services on our platform.
        </p>
      </LegalSection>

      <LegalSection id="consent" title="Cookie Consent">
        <p>
          When you first visit Magnetite, you will be presented with a cookie consent banner that allows you to:
        </p>
        <ul>
          <li>Accept all cookies</li>
          <li>Reject non-essential cookies</li>
          <li>Customize your cookie preferences</li>
        </ul>
        <p>
          <strong>Your Consent Choices:</strong> You can change your cookie preferences at any time by clicking the "Cookie Settings" link in our footer. Note that rejecting certain cookies may limit your ability to use some features of the platform.
        </p>
        <h3>Managing Cookies in Your Browser</h3>
        <p>
          Most web browsers allow you to control cookies through their settings. You can:
        </p>
        <ul>
          <li>View what cookies are stored on your device</li>
          <li>Delete all or specific cookies</li>
          <li>Block cookies from all or certain websites</li>
          <li>Block third-party cookies</li>
          <li>Clear all cookies when you close your browser</li>
        </ul>
        <p>
          Please note that blocking essential cookies will prevent Magnetite from functioning properly. For more information about managing cookies in your browser, visit:
        </p>
        <ul>
          <li>Google Chrome: <a href="https://support.google.com/accounts/answer/61416" target="_blank" rel="noopener noreferrer">https://support.google.com/accounts/answer/61416</a></li>
          <li>Mozilla Firefox: <a href="https://support.mozilla.org/en-US/kb/cookies-information-websites-store-on-your-computer" target="_blank" rel="noopener noreferrer">https://support.mozilla.org/en-US/kb/cookies-information-websites-store-on-your-computer</a></li>
          <li>Safari: <a href="https://support.apple.com/guide/safari/manage-cookies-sfri11471/mac" target="_blank" rel="noopener noreferrer">https://support.apple.com/guide/safari/manage-cookies-sfri11471/mac</a></li>
        </ul>
        <h3>Updates to This Policy</h3>
        <p>
          We may update this Cookie Policy from time to time to reflect changes in our practices or for other operational, legal, or regulatory reasons. Any updates will be posted on this page with a revised "Last updated" date.
        </p>
      </LegalSection>
    </LegalLayout>
  );
}
