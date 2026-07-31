<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# Assessments & research spikes

Standalone research documents that informed decisions recorded in
[the cross-repo backlog](#cross-repo-backlog) or
[Decentralization](#decentralization). Each is a primary-source-cited
investigation, not an implementation — none of them changed code by
themselves. Full text lives at the repo path given; this page is an index
with the verdict, not a copy.

| Assessment | Repo path | Verdict |
|---|---|---|
| Chain candidates | `docs/chain-candidates.md` | Sixteen chains plus an eleven-chain completeness sweep, evaluated on the offline-signing discriminator. Every yes/no is sourced; where a fact couldn't be established the cell says so. |
| Stellar history retention | `docs/stellar-history-retention.md` | Not a gate: `patala-stellar`'s `verify` splits into an offline cryptographic check (survives any retention loss) and a separate online Horizon check (degrades on pruning/reset) — see the doc for exactly which guarantee depends on which. |
| Sui item binding | `docs/sui-binding-spike.md` | Research spike only — no rail crate created, no code changed. Answers how a Sui payment could carry a tamper-evident item binding given Sui has no memo primitive. |
| `BlobStore` backend — Walrus vs evermesh vs local+HTTP | `docs/walrus-assessment.md` | Every capability claim about Walrus/Seal/evermesh is either sourced with a URL or explicitly marked unestablished. `magnetite`'s own current state is governed by [the status ledger](#status), not this document. |
| Folder transport (A23) | `docs/folder-transport-assessment.md` | FlowStock's folder sync is real, shipped Go code — but solves a different problem than magnetite has for packages or replay logs. The reusable part is the *shape* (content-addressed naming), not the code — no cross-product dependency was adopted. |
| TRACT-shaped storefront (A25) | `docs/a25-storefront-tract-assessment.md` | The backlog's framing was half right, half stale: module count (~36) confirmed, but "custodial" no longer holds — `marketplace.rs`'s USD path has been non-custodial since commit `87a3624`. What's real is *centralised* (one Postgres DB is sole catalogue authority), not custodial. Recommendation: do not migrate now — see the doc for why. |

See [Cross-repo backlog](#cross-repo-backlog) for the decisions these
assessments fed into, across magnetite and its sibling repositories.
