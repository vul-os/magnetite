import LegalLayout, { LegalSection } from '../components/LegalLayout';

const SECTIONS = [
  { id: 'acceptance', title: 'Acceptance of Terms' },
  { id: 'accounts', title: 'User Accounts' },
  { id: 'subscriptions', title: 'Subscriptions' },
  { id: 'game-access', title: 'Game Access and Payments' },
  { id: 'developer-terms', title: 'Developer Terms' },
  { id: 'developer-revenue', title: 'Developer Revenue Share' },
  { id: 'usage-limits', title: 'Usage Limits' },
  { id: 'intellectual-property', title: 'Intellectual Property' },
  { id: 'liability', title: 'Limitation of Liability' },
  { id: 'changes', title: 'Changes to Terms' },
];

export default function Terms() {
  return (
    <LegalLayout title="Terms of Service" lastUpdated="May 15, 2026" sections={SECTIONS}>
      <LegalSection id="acceptance" title="Acceptance of Terms">
        <p>
          By accessing or using Magnetite, you agree to be bound by these Terms of Service. If you do not agree to these terms, please do not use our platform. These terms apply to all users of Magnetite, including game developers, players, and content creators.
        </p>
        <p>
          You must be at least 18 years old to use Magnetite. By using the platform and agreeing to these terms, you represent and warrant that you have the legal capacity to enter into a binding contract.
        </p>
      </LegalSection>

      <LegalSection id="accounts" title="User Accounts">
        <p>
          To access certain features of Magnetite, you must create an account. You are responsible for maintaining the confidentiality of your account credentials and for all activities that occur under your account.
        </p>
        <p>When creating an account, you agree to:</p>
        <ul>
          <li>Provide accurate, current, and complete information</li>
          <li>Update your information to keep it accurate and current</li>
          <li>Not share your account credentials with any third party</li>
          <li>Notify us immediately of any unauthorized use of your account</li>
          <li>Not use another user's account without their explicit permission</li>
        </ul>
        <p>
          We reserve the right to suspend or terminate accounts that violate these terms or engage in prohibited activities.
        </p>
      </LegalSection>

      <LegalSection id="subscriptions" title="Subscriptions">
        <p>
          Magnetite offers subscription plans that provide access to additional features, games, or content. Subscription terms are displayed at the time of sign-up and are part of this agreement.
        </p>
        <p>
          <strong>Billing:</strong> Subscription fees are charged in advance on a recurring basis (weekly or monthly depending on your selected plan). All fees are in USD unless otherwise specified.
        </p>
        <p>
          <strong>Auto-Renewal:</strong> Your subscription automatically renews at the end of each billing period unless you cancel before the renewal date. By subscribing, you authorize us to charge your payment method for the next billing period.
        </p>
        <p>
          <strong>Cancellation:</strong> You may cancel your subscription at any time through your account settings. Cancellation takes effect at the end of your current billing period. You will retain access to subscription features until that date.
        </p>
        <p>
          <strong>Refunds:</strong> Subscriptions are non-refundable for the current billing period once charged. If you cancel before the end of your billing period, you will not be charged for the next period but will not receive a refund for the current period. We do not offer prorated refunds unless required by applicable law.
        </p>
        <p>
          <strong>Price Changes:</strong> We reserve the right to change subscription pricing. We will notify you of any price changes at least 30 days before they take effect. Price changes apply to the next billing cycle after the notice period.
        </p>
      </LegalSection>

      <LegalSection id="game-access" title="Game Access and Payments">
        <p>
          Magnetite provides access to games developed by third-party developers. When you purchase or access games through our platform, you agree to the payment terms specified at the time of purchase.
        </p>
        <p>
          <strong>Purchases:</strong> All game purchases are final and non-refundable unless otherwise stated by the developer or required by applicable law. Digital items and currencies purchased through Magnetite are for use within the platform only.
        </p>
        <p>
          <strong>Virtual Currency:</strong> Any virtual currency or tokens purchased through Magnetite have no real-world monetary value and cannot be exchanged for cash or any third-party services.
        </p>
        <p>
          <strong>Game Content:</strong> The availability of games, updates, and downloadable content may vary by region. Developers may modify or discontinue games at any time.
        </p>
      </LegalSection>

      <LegalSection id="developer-terms" title="Developer Terms">
        <p>
          If you are a game developer using Magnetite to distribute your games, you agree to the following additional terms:
        </p>
        <ul>
          <li>You must own or have the necessary rights to all content you publish</li>
          <li>You are responsible for setting your game's pricing and refund policies</li>
          <li>You agree to comply with all applicable laws and regulations regarding game content</li>
          <li>You will not engage in deceptive practices or false advertising</li>
          <li>You must respond to user support requests in a timely manner</li>
        </ul>
        <p>
          Developers retain ownership of their intellectual property. By publishing on Magnetite, you grant us a license to host and distribute your content on our platform.
        </p>
        <p>
          Revenue sharing and payment processing fees are as specified in your developer agreement.
        </p>
      </LegalSection>

      <LegalSection id="developer-revenue" title="Developer Revenue Share">
        <p>
          Developers earn revenue from games and content sold through Magnetite. This section explains how developer payouts are calculated and when payments are made.
        </p>
        <p>
          <strong>Payout Calculation:</strong> Developers receive a percentage of net revenue from their games, as specified in their developer agreement. Net revenue is calculated after deducting platform fees, payment processing fees, and any applicable taxes.
        </p>
        <p>
          <strong>Platform Fee:</strong> Magnetite retains a percentage of gross revenue as a platform fee for hosting, distribution, and platform maintenance. This fee is disclosed in your developer agreement and may vary.
        </p>
        <p>
          <strong>Payment Schedule:</strong> Payouts are processed on a schedule you choose during setup:
        </p>
        <ul>
          <li><strong>Weekly:</strong> Payouts are processed every Monday for the previous week's earnings, with a 7-day holding period to account for chargebacks or disputes.</li>
          <li><strong>Monthly:</strong> Payouts are processed on the 15th of each month for the previous month's earnings, with a 14-day holding period.</li>
        </ul>
        <p>
          <strong>Minimum Payout Threshold:</strong> Payouts are only processed when your earnings exceed the minimum threshold specified in your developer agreement. Earnings below this threshold roll over to the next payout period.
        </p>
        <p>
          <strong>Chargebacks and Refunds:</strong> If a player requests a chargeback or refund, the associated revenue may be clawed back from your earnings, even if payout has already been processed.
        </p>
        <p>
          <strong>Tax Responsibilities:</strong> Developers are responsible for their own tax obligations in their jurisdiction. Magnetite may be required to withhold taxes based on your location.
        </p>
      </LegalSection>

      <LegalSection id="usage-limits" title="Usage Limits">
        <p>
          Magnetite may impose usage limits to ensure fair access for all users and platform stability. These limits help us maintain service quality and prevent abuse.
        </p>
        <p>
          <strong>Fair Use Policy:</strong> We expect all users to use the platform reasonably. Excessive usage that impacts platform performance or denies access to other users may be restricted. What constitutes "excessive" is determined at our sole discretion based on normal usage patterns.
        </p>
        <p>
          <strong>Resource Limits:</strong> Certain features or actions may be subject to limits on usage frequency, data transfer, storage, or computational resources. These limits are disclosed where applicable.
        </p>
        <p>
          <strong>What Happens If Limits Are Exceeded:</strong> If you exceed usage limits:
        </p>
        <ul>
          <li>We may temporarily restrict access to certain features</li>
          <li>You may experience reduced performance or throttling</li>
          <li>We may request you upgrade to a higher tier plan if available</li>
          <li>In severe cases, we may suspend or terminate your account</li>
        </ul>
        <p>
          <strong>No Guarantee of Uninterrupted Access:</strong> Usage limits are applied at our discretion. We do not guarantee uninterrupted access to any feature or service.
        </p>
        <p>
          If you believe you have been incorrectly limited, you may contact support to request a review.
        </p>
      </LegalSection>

      <LegalSection id="intellectual-property" title="Intellectual Property">
        <p>
          Magnetite and its original content, features, and functionality are owned by AnomalyCo and are protected by international copyright, trademark, patent, trade secret, and other intellectual property laws.
        </p>
        <p>
          You retain ownership of any content you create and publish on Magnetite. By publishing content, you grant us a non-exclusive, transferable, sublicensable, royalty-free license to use, reproduce, modify, distribute, and display your content in connection with operating the platform.
        </p>
        <p>
          You agree not to copy, modify, or distribute any Magnetite trademarks, logos, or copyrighted materials without our prior written consent.
        </p>
      </LegalSection>

      <LegalSection id="liability" title="Limitation of Liability">
        <p>
          To the maximum extent permitted by law, Magnetite and its parent company, affiliates, officers, employees, agents, and partners shall not be liable for any indirect, incidental, special, consequential, or punitive damages, including but not limited to loss of profits, data, or other intangible losses resulting from:
        </p>
        <ul>
          <li>Your use of or inability to use the platform</li>
          <li>Any unauthorized access to your account or data</li>
          <li>Any game defects, bugs, or technical issues</li>
          <li>Any actions of other users or third parties</li>
          <li>Any content obtained through the platform</li>
        </ul>
        <p>
          Our total liability for any claims arising from these terms or your use of Magnetite shall not exceed the amount you paid us in the past twelve months, if any.
        </p>
      </LegalSection>

      <LegalSection id="changes" title="Changes to Terms">
        <p>
          We reserve the right to modify or replace these Terms of Service at any time at our sole discretion. If a revision is material, we will provide at least 30 days' notice prior to any new terms taking effect.
        </p>
        <p>
          What constitutes a material change will be determined at our sole discretion. By continuing to access or use Magnetite after any revisions become effective, you agree to be bound by the revised terms.
        </p>
        <p>
          If you do not agree to the new terms, you are no longer authorized to use Magnetite.
        </p>
      </LegalSection>
    </LegalLayout>
  );
}
