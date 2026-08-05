import LegalLayout, { LegalSection } from '../components/LegalLayout';

const SECTIONS = [
  { id: 'information-collect', title: 'Information We Collect' },
  { id: 'how-we-use', title: 'How We Use Information' },
  { id: 'information-sharing', title: 'Information Sharing' },
  { id: 'data-retention', title: 'Data Retention' },
  { id: 'your-rights', title: 'Your Rights' },
  { id: 'security', title: 'Security Measures' },
  { id: 'contact', title: 'Contact' },
];

export default function Privacy() {
  return (
    <LegalLayout title="Privacy Policy" lastUpdated="May 15, 2026" sections={SECTIONS}>
      <LegalSection id="information-collect" title="Information We Collect">
        <p>
          We collect information you provide directly to us, including when you create an account, make a purchase, or contact us for support. This includes:
        </p>
        <ul>
          <li><strong>Account Information:</strong> Name, email address, username, password, and profile picture</li>
          <li><strong>Payment Information:</strong> Billing address, payment method details, and transaction history</li>
          <li><strong>Profile Data:</strong> Games played, achievements, leaderboard rankings, and friend lists</li>
          <li><strong>Communications:</strong> Messages you send to us or other users through our platform</li>
          <li><strong>Technical Data:</strong> IP address, device type, browser information, and operating system</li>
        </ul>
        <p>
          We also collect information automatically when you use Magnetite, including gameplay statistics, session duration, and crash reports.
        </p>
      </LegalSection>

      <LegalSection id="how-we-use" title="How We Use Information">
        <p>We use the information we collect to:</p>
        <ul>
          <li>Provide, maintain, and improve our platform and services</li>
          <li>Process transactions and send related information</li>
          <li>Send you technical notices, updates, and support messages</li>
          <li>Respond to your comments, questions, and customer service requests</li>
          <li>Communicate with you about products, services, and events</li>
          <li>Monitor and analyze trends, usage, and engagement patterns</li>
          <li>Detect, investigate, and prevent fraudulent or unauthorized activities</li>
          <li>Personalize and improve your experience on the platform</li>
        </ul>
      </LegalSection>

      <LegalSection id="information-sharing" title="Information Sharing">
        <p>
          We do not sell, trade, or otherwise transfer your personal information to third parties except as described in this policy. We may share your information with:
        </p>
        <ul>
          <li><strong>Service Providers:</strong> Third parties who perform services on our behalf, such as payment processing, data analysis, and customer support</li>
          <li><strong>Game Developers:</strong> When you purchase or play a game, the developer may receive information about your gameplay</li>
          <li><strong>Legal Requirements:</strong> When required by law, subpoena, or court order, or when we believe disclosure is necessary to protect our rights</li>
          <li><strong>Business Transfers:</strong> In connection with a merger, acquisition, or sale of assets</li>
          <li><strong>With Your Consent:</strong> When you have given us permission to share your information</li>
        </ul>
        <p>
          We may share aggregated or anonymized information that cannot reasonably identify you with third parties.
        </p>
      </LegalSection>

      <LegalSection id="data-retention" title="Data Retention">
        <p>
          We retain your information for as long as your account is active or as needed to provide you services. We also retain your information as necessary to comply with legal obligations, resolve disputes, and enforce agreements.
        </p>
        <p>
          Upon account deletion, we will delete your personal information within 90 days, except where retention is required by law. Some information may be retained in anonymized or aggregated form.
        </p>
        <p>
          Gameplay statistics and achievements may be retained indefinitely for leaderboard and historical records purposes, even after account deletion.
        </p>
      </LegalSection>

      <LegalSection id="your-rights" title="Your Rights">
        <p>
          Depending on your location, you may have certain rights regarding your personal information:
        </p>
        <h3>General Rights</h3>
        <ul>
          <li><strong>Access:</strong> Request a copy of the personal information we hold about you</li>
          <li><strong>Correction:</strong> Request correction of inaccurate or incomplete information</li>
          <li><strong>Deletion:</strong> Request deletion of your personal information</li>
          <li><strong>Objection:</strong> Object to certain processing of your information</li>
          <li><strong>Data Portability:</strong> Request export of your data in a machine-readable format</li>
        </ul>
        <h3>GDPR Rights (European Economic Area)</h3>
        <p>
          If you are located in the EEA, you have additional rights under the General Data Protection Regulation:
        </p>
        <ul>
          <li>The right to withdraw consent at any time</li>
          <li>The right to restrict processing of your data</li>
          <li>The right to lodge a complaint with a data protection authority</li>
        </ul>
        <h3>CCPA Rights (California)</h3>
        <p>
          California residents have the right to know what personal information is collected, request deletion, and opt-out of the sale of their personal information. We do not sell personal information.
        </p>
        <p>
          To exercise any of these rights, please contact us at privacy@magnetite.gg.
        </p>
      </LegalSection>

      <LegalSection id="security" title="Security Measures">
        <p>
          We implement appropriate technical and organizational measures to protect your personal information against unauthorized access, alteration, disclosure, or destruction. These measures include:
        </p>
        <ul>
          <li>Encryption of data in transit using TLS/SSL</li>
          <li>Encryption of sensitive data at rest</li>
          <li>Regular security assessments and penetration testing</li>
          <li>Access controls and authentication requirements</li>
          <li>Employee training on data protection practices</li>
        </ul>
        <p>
          While we strive to protect your information, no method of transmission over the Internet or electronic storage is 100% secure. We cannot guarantee absolute security.
        </p>
      </LegalSection>

      <LegalSection id="contact" title="Contact">
        <p>
          If you have any questions about this Privacy Policy or our data practices, please contact us:
        </p>
        <div className="contact-info">
          <p>
            <strong>Email:</strong> privacy@magnetite.gg<br />
            <strong>Mail:</strong> Magnetite Privacy Team<br />
            AnomalyCo, Inc.<br />
            123 Blockchain Avenue<br />
            San Francisco, CA 94102<br />
            United States
          </p>
        </div>
        <p>
          We will respond to your request within 30 days. For data protection inquiries in the EEA, you may contact our Data Protection Officer at dpo@magnetite.gg.
        </p>
      </LegalSection>
    </LegalLayout>
  );
}
