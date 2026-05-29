export default function SocialProof({
  userCount = 12500,
  gameCount = 340,
}) {
  const formatNumber = (n) => {
    if (n >= 1000) return (n / 1000).toFixed(1) + 'k+';
    return n.toString();
  };

  return (
    <div className="social-proof">
      <div className="social-proof-item">
        <span className="social-proof-value">{formatNumber(userCount)}</span>
        <span className="social-proof-label">developers</span>
      </div>
      <div className="social-proof-divider" />
      <div className="social-proof-item">
        <span className="social-proof-value">{formatNumber(gameCount)}</span>
        <span className="social-proof-label">games hosted</span>
      </div>
    </div>
  );
}