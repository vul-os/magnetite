# Security Policy

Magnetite is a decentralized game platform — players bring their own servers for
capacity, and account ownership is non-custodial. Security reports are taken
seriously and handled with priority.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

- Preferred: [GitHub private vulnerability reporting](https://github.com/vul-os/magnetite/security/advisories/new) on `vul-os/magnetite`.
- Alternatively, email **vulosorg@gmail.com** with `[magnetite security]` in the subject.

Include what you can: affected component (a seam, the marketplace/payment
service, the matchmaking or fleet layer, a game template), reproduction steps,
and impact as you understand it. You'll get an acknowledgement within **72
hours** and a status update at least every **14 days** until resolution. Please
give a reasonable window to ship a fix before public disclosure — we'll credit
you in the release notes unless you'd rather stay anonymous.

## Scope

Especially interested in:

- **Non-custodial account & key handling** — any path that displays, exports,
  logs, or exfiltrates secret material, or lets one player act as another.
- **Payment & marketplace seams** — value moved without authorization, receipt
  or chain-binding forgery, double-spend.
- **Bring-your-own-server capacity** — a hostile capacity provider affecting
  players it should not, or escaping its sandbox.
- **Seam boundaries** — any pluggable seam (comms, payment, identity) whose
  default implementation can be coerced into acting against the player.

Out of scope: vulnerabilities requiring an already-compromised host or a
player's own device, and issues in third-party services a player configures
(their chosen comms bridge, their own server host).

## Supported versions

Pre-1.0: only the latest release (and `main`) receives fixes.
