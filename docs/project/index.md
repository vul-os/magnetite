# Project History

Program records: what was decided, what was audited, what landed. These are
**historical documents**, not user documentation. They are kept because the
record of a reversed decision is more valuable than a tidy directory — several
of the decisions below were later overturned, and the reversal only makes sense
next to the original.

For what Magnetite *is today*, start at [the documentation hub](../index.md) and
[`DECENTRALIZATION.md`](../../DECENTRALIZATION.md) at the repo root.

| Document | Status | What it is |
|----------|--------|------------|
| [DECENTRALIZATION_PROGRESS.md](./DECENTRALIZATION_PROGRESS.md) | **Active** | Append-only log of what actually landed in the decentralization program. Agents append here. |
| [DECISIONS.md](./DECISIONS.md) | **Partly superseded** | Autonomous build decisions. Records the 2026-06-01 fiat-only payments pivot, which has **since been reversed** — payments are now non-custodial with no fiat on-ramp. |
| [AUDIT.md](./AUDIT.md) | **Historical** | Wiring & security audit (2026-06-01). Its critical/high findings were resolved; kept as the record of that wave. |
| [GAPS.md](./GAPS.md) | **Stale** | Gap re-audit (2026-05-30). Predates the decentralization redesign; describes the platform in its "one big backend" shape. |
| [TASKS.md](./TASKS.md) | **Stale** | Implementation task checklist. Predates the decentralization redesign. |
| [roadmap.md](./roadmap.md) | **Stale** | Phase-based platform roadmap from the centralized era. |

## Why these moved

They used to sit at the repository root. The root now carries only what tooling
and GitHub expect to find there — `README.md`, `CONTRIBUTING.md`,
`CHANGELOG.md`, `LICENSE` — plus `DECENTRALIZATION.md`, which stays at the root
because it is the active anchor spec that other repositories and agent prompts
reference by path.
