# The zero-third-party path (A22)

`ALIGNMENT.md` §2 item 9 asked for the self-hostable tracker to be documented
as "the zero-third-party path." That framing is too narrow to answer honestly
on its own — the tracker is one of several places a third party can enter (or
not enter) a Magnetite deployment. This page walks the whole path end to end:
discovery, reachability, package distribution, and payments — what needs
nobody else, and where a third party is genuinely unavoidable if you want
more than that.

**The conclusion, stated up front so it cannot be buried:** LAN-only play
needs no external service of any kind, today, as shipped. The zero-third-party
path stops being *fully* zero the moment you want a stranger on the public
internet, using an ordinary browser, to find and join your server without you
having handed them the address yourself. At that point a certificate
authority and a domain registrar (or, if you'd rather not run a proxy
yourself, a tunnel) are the honest floor — not a corner this project cut.

## 1. Discovery — do you even need a tracker?

**The default needs nothing external.** With no `TRACKER_URL` set,
`magnetite node` uses `LanDiscovery` (mDNS) — verified directly in code:
`magnetite-runtime/src/tracker.rs`'s `from_env` returns `None` whenever
`TRACKER_URL` is unset or blank, and callers treat `None` as "use LAN
discovery." Nothing is contacted over the internet. This is the true
zero-third-party case, and it is also the *common* case: if you are hosting
your own game for your own players, you already know your own domain —
give it to players directly and they connect via `wss://your-node.example.com`
(see [Hosting a server → Running players over `wss://`](hosting-a-server.md#running-players-over-wss)).
**No tracker is needed at all for this**, full stop — a tracker only matters
when a player needs to *discover* a server whose address they don't already
have, i.e. a public "browse servers" experience spanning independent
operators.

**If you do want that public server-browser experience**, the situation is
more nuanced than "anyone can run a dumb tracker":

- The *client* side (`TrackerDiscovery` / `HttpTrackerClient` in
  `magnetite-runtime/src/tracker.rs`) is genuinely thin and swappable — it is
  a plain HTTP POST/GET/DELETE against any base URL, with no dependency on
  who runs the other end.
- The *server* side — the actual implementation of
  `POST /api/v1/discovery/announce`, `GET /api/v1/discovery/sessions`,
  `DELETE /api/v1/discovery/announce` — has exactly one implementation in
  this repo, and it lives in `backend/src/api/discovery.rs`, part of the same
  `magnetite-backend` binary as ~30 other route modules (auth, wallet,
  marketplace, oauth, matchmaking, …; see `backend/src/main.rs`), and it needs
  a Postgres pool: the `discovery_ads` table is defined inside
  `backend/migrations/20250119000000_baseline.sql`, the single baseline
  migration that also creates every other table the monolith uses. **Today,
  self-hosting "just a tracker" in practice means standing up the whole
  backend** (plus Postgres) — even though you want three routes out of
  dozens.
- This is not a hard architectural coupling: `discovery::router(pool: PgPool)`
  is `pub`, takes nothing but a Postgres connection pool, and could be
  composed into a much smaller standalone binary by anyone willing to write
  ~20 lines wiring it up. **That extraction has not been done or tested
  anywhere in this repo.** Do not claim a minimal tracker binary exists —
  only the full monolith has ever been run as the discovery server. This is
  the accurate state of "self-hostable tracker" today: architecturally
  possible, not yet built as a lightweight artifact.
- **A real security caveat for anyone who does build that minimal tracker:**
  the reference server verifies every announce's signature and lease window
  on write (`verify_announce` in `backend/src/api/discovery.rs`) — but the
  *reading* side does **not** re-verify anything. `HttpTrackerClient::find`
  and `parse_sessions` (`magnetite-runtime/src/tracker.rs`) deserialize a
  tracker's response straight into the bare, unsigned `SessionAd` — there is
  no `SignedAd::verify` call anywhere on that path. So a querying node trusts
  whatever a tracker hands back, entirely. A from-scratch tracker
  reimplementation that skips the write-side signature check would let
  anyone spoof a session ad, with nothing on the client side to catch it. A
  smaller, self-hostable tracker is a fine idea; one that drops
  `verify_announce`'s checks to get there is a security regression, not
  merely a smaller service — the "dumb, swappable HTTP tracker" framing in
  `docs/hosting-a-server.md` is accurate about redundancy and disposability,
  but "dumb" must not mean "unauthenticated."

## 2. Reachability — getting a player's browser to the node

- **LAN-only:** zero third party, unconditionally. A player on the same
  network opens a page served over plain `http://` and connects over plain
  `ws://` — the browser's mixed-content blocking never triggers, because the
  page itself is not `https://`. No domain, no certificate, no proxy, nothing
  external.
- **Public play, from an ordinary browser:** the browser's own secure-context
  rule forces `wss://` unconditionally the moment the page is `https://` (see
  [Hosting a server](hosting-a-server.md#running-players-over-wss), verified
  end to end there). That means a real hostname and a certificate the
  browser trusts. Of the three routes documented there:
  - **A reverse proxy (Caddy) obtaining a certificate from
    [Let's Encrypt](https://letsencrypt.org/)** is the recipe already
    verified (locally, against Caddy's own local CA rather than a live
    issuance — see the caveat already recorded there). Let's Encrypt is
    free and fully automated, but it *is* a third party: an outage or a
    policy change there is outside the operator's control, and the domain
    itself has to come from a registrar — also a third party. **This is the
    one third-party dependency that is genuinely hard to avoid if the
    general public is meant to reach the server as `https://` clients.**
  - **A self-signed certificate against your own private root**, given to
    players out of band, is zero third party — the same *shape* of trust
    A19's own verification used (a client that explicitly trusts a
    non-publicly-issued root, not `--insecure`). It does not scale to the
    general public: each player's browser has to be made to trust that root
    first (import it, or click through a per-host exception), which is
    workable for a small invited group and a bad experience for anyone else.
  - **A tunnel** (ngrok, cloudflared, or the suite's own Ephor) is already
    labelled a third party in `docs/hosting-a-server.md` — "the tunnel
    operator is a content-visible L7 hop in the path" — and that stands
    unchanged here. It substitutes one third party (the tunnel operator) for
    another (a CA + registrar), and is never a default.
- **Node-to-node (cluster handoff):** no TLS is attempted in-process at all —
  the existing guidance is to run it over a network you already trust (LAN,
  VPN, or a tunnel). A self-hosted VPN — plain
  [WireGuard](https://www.wireguard.com/) between boxes you control, with no
  external coordination service — is zero third party. A *hosted*
  coordination-plane VPN such as [Tailscale](https://tailscale.com/)'s
  default service is a third party unless you run its open-source
  self-hosted control-server alternative,
  [Headscale](https://github.com/juanfont/headscale), yourself. Magnetite
  ships and requires neither; this is purely an operator choice sitting
  outside this repo's code, noted here only so the tradeoff is not hidden.

## 3. Package distribution — A23's answer, restated for this question

Already assessed in full in `docs/folder-transport-assessment.md`; the
relevant conclusion for *this* question: Magnetite packages are
content-addressed and signed (`magnetite-seams/src/package.rs`), so **any**
existing file-sync tool already replicates them correctly, verifiably, with
zero new Magnetite code and zero live socket. Whether that stays
zero-third-party depends entirely on which tool and how it's configured:

- **rsync over your own SSH, or a literal USB stick** — zero third party,
  unconditionally. Nothing but the two machines (or the stick) is involved.
- **[Syncthing](https://syncthing.net/)** is itself open-source and
  self-hostable peer-to-peer, but **its defaults are not zero-third-party**:
  `globalAnnounceEnabled` and `relaysEnabled` are both `true` out of the box,
  meaning a fresh install announces to and can route through Syncthing's own
  public discovery/relay servers whenever two devices cannot reach each
  other directly
  ([Syncthing relaying docs](https://docs.syncthing.net/users/relaying.html):
  "Relaying is enabled by default but will only be used if two devices are
  unable to communicate directly with each other"). Setting both
  `globalAnnounceEnabled` and `relaysEnabled` to `false` and configuring
  direct device addresses removes this
  ([Syncthing config reference](https://docs.syncthing.net/users/config.html)).
  **"Just use Syncthing" quietly reintroduces a third party unless this is
  turned off** — worth stating plainly rather than assuming the tool is
  neutral by default.
- A commercial cloud-sync folder (Dropbox, Google Drive, etc.) is a third
  party by definition; it was listed in the folder-transport assessment as a
  workable *carrier* precisely because packages are self-verifying, not
  because using one is free of a third party.

## 4. Payments — the standing truth, restated here for completeness

Magnetite has **never settled a payment itself**. `PAYMENT_RAIL=mock` is the
default and is fully offline — no third party, no network call, signed
receipts only. A real chain rail exists for Stellar
(`magnetite-stellar-rail`), and using it inherently talks to a public chain's
RPC endpoint (Horizon) to confirm settlement — a public network is not a
private third-party vendor in the same sense as a hosted SaaS, but it is
still an external system this repo does not operate; see
`docs/stellar-history-retention.md` for exactly which verification steps
degrade, and how, if that RPC is unreachable. This is unavoidable *if* you
want on-chain-verified settlement rather than the offline mock rail — it is
not a corner cut, it is what "chain-verified" means.

## 5. The honest boundary, at a glance

| Layer | Zero-third-party path exists? | Where a third party creeps in if you want more |
|---|---|---|
| Discovery (finding a server) | **Yes** — `LanDiscovery` is the default, and most self-hosters don't need a tracker at all (see §1) | A public "browse servers" UX needs a tracker; the only implementation today is the full backend monolith — still your own box, but heavier than "dumb tracker" suggests, and a hand-built minimal one must not skip its signature check |
| Reachability, LAN | **Yes**, unconditionally | — |
| Reachability, public browser | Partial — a self-signed cert works for a small trusted group | A CA (Let's Encrypt) + a domain registrar for the general public; a tunnel if you'd rather not run a proxy at all |
| Node-to-node cluster | **Yes**, on a LAN/VPN you run yourself | A hosted VPN coordination plane (Tailscale), unless you self-host its alternative (Headscale) |
| Package distribution | **Yes** — rsync/USB/a correctly-configured Syncthing peer | Syncthing's own default discovery/relay servers, if left on; any commercial cloud-sync folder, by definition |
| Payments | **Yes** — `PAYMENT_RAIL=mock`, fully offline | A real chain rail talks to a public chain's RPC by definition |

## What could not be established

- Whether Syncthing's discovery/relay servers are operated by the Syncthing
  project itself or by independent volunteers — the cited docs describe the
  *default-on* behavior and how to disable it, not who currently operates
  the public infrastructure; not verified further here.
- Whether a minimal, extracted tracker binary (composing
  `backend::api::discovery::router` without the rest of the monolith) has
  ever been attempted by anyone outside this repo — could not establish;
  nothing in this codebase or its history suggests it has.
