import './Landing.css';

/**
 * Real product screenshots of the running app, captured from the current build
 * (after the fabricated-data removal). Each surface ships a dark and a light
 * variant served same-origin from /screenshots/; CSS shows the one that matches
 * the active theme. No external image hosts — see DESIGN.md §7.
 */
const SHOTS = [
  {
    name: 'servers',
    kicker: '// DISCOVERY',
    title: 'Server browser',
    caption:
      'Discovery is a phonebook, never an authority. Every session is self-advertised — signed by the node key, addressed by content hash — and the readout marks exactly what it can and cannot verify.',
    alt: 'Magnetite server browser listing self-advertised game sessions, each labelled as checkable or unverifiable, with node keys, content hashes, capacity and ping.',
  },
  {
    name: 'deploy',
    kicker: '// SHIP',
    title: 'Game deployment',
    caption:
      'Connect a repository and push Rust. Builds run on your own infrastructure and the play manifest goes live — no cloud in the middle, and the platform takes no cut.',
    alt: 'Magnetite game deployment screen: connect GitHub, then a three-step build-and-deploy pipeline that runs on self-hosted infrastructure.',
  },
  {
    name: 'studio',
    kicker: '// AUTHOR',
    title: 'Game studio',
    caption:
      'Start from a template that implements the authoritative-sim trait, get the CLI, and scaffold the Rust crate — from a sixteen-player room to AAA-scale sharding.',
    alt: 'Magnetite game studio template gallery: arena shooter, platformer, FPS, motorsport, RTS and blank-slate templates with player counts, tick rate and topology.',
  },
];

function ShotFrame({ shot }) {
  return (
    <figure className="shot">
      <div className="shot-frame">
        <div className="shot-bar" aria-hidden="true">
          <span className="shot-dot" />
          <span className="shot-dot" />
          <span className="shot-dot" />
          <span className="shot-bar-label">magnetite · {shot.name}</span>
          <span className="shot-live"><span className="shot-live-dot" />app</span>
        </div>
        <div className="shot-media">
          <img
            className="shot-img shot-dark"
            src={`/screenshots/${shot.name}-dark.png`}
            alt={shot.alt}
            width="1280"
            height={shot.name === 'servers' ? 980 : 900}
            decoding="async"
          />
          <img
            className="shot-img shot-light"
            src={`/screenshots/${shot.name}-light.png`}
            alt=""
            aria-hidden="true"
            width="1280"
            height={shot.name === 'servers' ? 980 : 900}
            decoding="async"
          />
        </div>
      </div>
      <figcaption className="shot-caption">
        <span className="shot-caption-kicker m-sm">{shot.kicker}</span>
        <span className="shot-caption-title">{shot.title}</span>
        <span className="shot-caption-body">{shot.caption}</span>
      </figcaption>
    </figure>
  );
}

export default function ProductShowcase() {
  const [hero, ...rest] = SHOTS;

  return (
    <section className="showcase-section" aria-labelledby="showcase-heading">
      <div className="container">
        <div className="section-header-centered">
          <span className="kicker">// THE PRODUCT</span>
          <h2 id="showcase-heading" className="section-heading">
            A control surface for a{' '}
            <span className="gradient-text">verifiable machine</span>
          </h2>
          <p className="section-lead">
            Not a storefront — an instrument panel. Every screen is honest about what it
            can prove and what it cannot.
          </p>
        </div>

        <div className="showcase-hero">
          <ShotFrame shot={hero} />
        </div>

        <div className="showcase-grid">
          {rest.map((shot) => (
            <ShotFrame key={shot.name} shot={shot} />
          ))}
        </div>
      </div>
    </section>
  );
}
