export default function PricingCard({ title, price, features, cta, highlight, isCurrentPlan, limitations, type }) {
  return (
    <div className={`pricing-card ${highlight ? 'pricing-card-highlight' : ''} ${isCurrentPlan ? 'pricing-card-current' : ''}`}>
      {highlight && !isCurrentPlan && <div className="pricing-card-badge">Most Popular</div>}
      {isCurrentPlan && <div className="pricing-card-badge current">Current Plan</div>}
      <div className="pricing-card-header">
        <h3 className="pricing-card-title">{title}</h3>
        <div className="pricing-card-price">{price}</div>
      </div>
      <ul className="pricing-card-features">
        {features.map((feature, i) => (
          <li key={i} className="pricing-card-feature">
            <span className="pricing-check">✓</span>
            <span>{feature}</span>
          </li>
        ))}
      </ul>
      {limitations && limitations.length > 0 && (
        <ul className="pricing-card-limitations">
          {limitations.map((limitation, i) => (
            <li key={i} className="pricing-card-limitation">
              <span className="pricing-x">✗</span>
              <span>{limitation}</span>
            </li>
          ))}
        </ul>
      )}
      <a 
        href={cta.href} 
        className={`btn ${highlight && !isCurrentPlan ? 'btn-primary' : 'btn-secondary'} pricing-card-cta`}
        style={isCurrentPlan ? { backgroundColor: '#6b7280', borderColor: '#6b7280', cursor: 'default' } : {}}
        onClick={isCurrentPlan ? (e) => e.preventDefault() : undefined}
      >
        {cta.label}
      </a>
    </div>
  );
}