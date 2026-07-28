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

## Verifying a downloaded release

Every `v*` release publishes a `SHA256SUMS` manifest covering **every** asset
attached to it, plus a sigstore **build provenance attestation** minted from
the release workflow's OIDC identity — a short-lived certificate, not a
long-lived key someone has to hold and rotate. It binds the bytes to this
repository's release workflow at a commit.

Check a binary before you run it:

```sh
curl -fsSLO https://raw.githubusercontent.com/vul-os/magnetite/vX.Y.Z/scripts/verify.sh
bash verify.sh --tag vX.Y.Z magnetite-X.Y.Z-linux-x86_64            # digest
bash verify.sh --tag vX.Y.Z --attest magnetite-X.Y.Z-linux-x86_64   # + provenance
```

`verify.sh` needs only `curl` and `sha256sum`/`shasum`. It has two outcomes:
verified, or a non-zero exit with a diagnostic naming what was wrong — a
missing manifest (3), an HTML page where the manifest was expected (4), an
empty or malformed manifest (5), no entry for the asset (6), an unfetchable
artifact (7), a truncated download (8), a digest mismatch (9). There is no
skip flag, and a missing `SHA256SUMS` is never treated as "nothing to check":
a verifier that shrugs at a 404 prints a line that looks like verification
while checking nothing, which is worse than no verifier at all. `--attest`
needs the `gh` CLI; a run without it prints that provenance was **not**
checked, so a pass never implies more than it checked.

Container images are addressed by their own registry digest
(`docker pull magnetite/magnetite@sha256:...`) and are deliberately not listed
in `SHA256SUMS`. These binaries are **not** OS code-signed.

## Supported versions

Pre-1.0: only the latest release (and `main`) receives fixes.
