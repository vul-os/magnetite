import LegalLayout, { LegalSection } from '../components/LegalLayout';

const SECTIONS = [
  { id: 'acceptance', title: 'Acceptance of Terms' },
  { id: 'accounts', title: 'User Accounts' },
  { id: 'payments', title: 'Payments' },
  { id: 'game-access', title: 'Game Access and Payments' },
  { id: 'developer-terms', title: 'Developer Terms' },
  { id: 'developer-revenue', title: 'Developer Payments' },
  { id: 'usage-limits', title: 'Usage Limits' },
  { id: 'intellectual-property', title: 'Intellectual Property' },
  { id: 'liability', title: 'Limitation of Liability' },
  { id: 'changes', title: 'Changes to Terms' },
];

export default function Terms() {
  return (
    <LegalLayout title="Terms of Service" lastUpdated="July 20, 2026" sections={SECTIONS}>
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

      <LegalSection id="payments" title="Payments">
        <p>
          Magnetite is a distributed, self-hostable platform: you run your own node, or connect to one an operator runs. The platform itself charges you nothing. There is no subscription, no tier fee, no metered billing, and no platform-held balance &mdash; we take custody of no funds and process no charge against you.
        </p>
        <p>
          <strong>The only money movement is optional and direct.</strong> Any payment happens wallet-to-wallet between users, with no intermediary holding the funds &mdash; for example, a player paying a developer for a game (see Game Access and Payments below), or a hosting fee paid directly to the operator who runs a server. You link an Ed25519 wallet address; a checkout settles the transfer on a third-party payment rail we do not operate and returns a signed receipt as the record.
        </p>
        <p>
          <strong>No custody, so no payouts or clawbacks.</strong> Because the platform never holds funds, there is nothing for us to pay out, freeze, refund, or claw back, and no card or bank details for us to store. A completed wallet settlement is final; any recourse for a disputed payment is a matter between the users who transacted, on whatever terms they agreed. Chargebacks do not apply to non-custodial wallet settlement.
        </p>
      </LegalSection>

      <LegalSection id="game-access" title="Game Access and Payments">
        <p>
          Magnetite provides access to games published by third-party developers. When you purchase or unlock a game or item through the platform, you agree to the payment terms shown at the time of purchase.
        </p>
        <p>
          <strong>Wallet-to-wallet purchases:</strong> Purchases settle directly from your wallet to the developer&rsquo;s wallet (and, where applicable, the operator&rsquo;s) in a single atomic transfer. The platform never holds the funds. Your entitlement is the signed receipt the payment rail returns, keyed to your wallet and the item purchased; the node reads that receipt to grant access.
        </p>
        <p>
          <strong>Finality:</strong> Because settlement is wallet-to-wallet and non-custodial, completed purchases are final. We hold no balance from which a refund could be issued and cannot reverse a rail settlement. Chargebacks do not apply to non-custodial wallet payments. Any recourse is a matter between you and the developer or operator you paid, on whatever terms they offer.
        </p>
        <p>
          <strong>Protocol fee:</strong> A checkout may carry a protocol fee, expressed in basis points and itemised on the receipt. It defaults to zero, in which case the developer or operator receives the full subtotal.
        </p>
        <p>
          <strong>Points and XP:</strong> Any in-platform points or experience are off-chain records, not money. They have no monetary value, are not a stored balance, and cannot be exchanged for cash or transferred off the platform.
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
          Payment splits and any protocol fee settle directly at checkout, wallet-to-wallet, as described in Developer Payments below and in your developer agreement.
        </p>
      </LegalSection>

      <LegalSection id="developer-revenue" title="Developer Payments">
        <p>
          Developers are paid for games and content sold through Magnetite. Payment is non-custodial: the platform never holds developer funds and operates no payout system. This section explains how developers are paid.
        </p>
        <p>
          <strong>Direct settlement at checkout:</strong> When a player checks out, the payment rail splits the transfer and pays the developer&rsquo;s wallet directly, in the same atomic settlement. There is no platform-held balance that accrues to you and no payout to request — you are paid at the moment of sale, wallet-to-wallet.
        </p>
        <p>
          <strong>No holding period, no minimum threshold:</strong> Because nothing is held on your behalf, there is no holding period before funds are released, no minimum balance to reach before a payout, and no rollover of earnings between periods. These concepts do not apply to non-custodial settlement.
        </p>
        <p>
          <strong>No clawbacks:</strong> Settlements are wallet-to-wallet and final. The platform holds no balance from which prior earnings could be clawed back, and chargebacks do not apply to non-custodial wallet payments. We cannot and do not reverse a completed settlement.
        </p>
        <p>
          <strong>Protocol fee and splits:</strong> A checkout may carry a protocol fee, expressed in basis points and itemised on the receipt; it defaults to zero. Any operator hosting fee is likewise a direct payment to the operator, not a balance held by the platform. Aside from that optional protocol fee, the platform takes no cut of your sales.
        </p>
        <p>
          <strong>Tax Responsibilities:</strong> Developers are responsible for their own tax obligations in their jurisdiction. Because payments settle directly to your wallet and the platform holds no funds, the platform does not withhold taxes on your behalf; you are responsible for reporting and remitting any tax due on amounts you receive.
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
