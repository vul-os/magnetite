# Hosting a server

**Bring any server. It scales to your hardware. No cloud required.**

Magnetite collapses the platform backend and the game-server runtime into one
generic `magnetite` node binary. There is no separate provisioning API to call
and no central fleet to request capacity from — you point the binary at a box
you already have, and it takes it from there.

## The node measures itself

On start, a node measures its own hardware — cores, RAM, bandwidth — and
advertises that as `Capacity` to the discovery layer (see
[Architecture](architecture.md#the-seams)). Nothing about player capacity is a
config constant you have to guess and tune; it is **emergent from the box**.
Give the node more cores, and it runs more shards. Give it a faster uplink,
and it advertises a higher player ceiling.

## Shards, not fixed rooms

A world is a set of **shards** — a spatial cell, a room, an instance. Players
live inside a shard; crossing a boundary is a handoff to a neighboring shard.
A game only has to declare *how to partition its state* through a `Shardable`
trait; a pluggable `ShardScheduler` places shards onto whatever capacity
exists. A single box runs as many shards as it can hold. This is what makes
the same game code walk the full topology ladder:

| Topology | Player count | Where it runs |
|----------|-------------|----------------|
| `SingleRoom` | up to ~16 | one process, on your laptop |
| `Dedicated` | up to ~256 | one authoritative server, one box |
| `Sharded` | AAA / unbounded | many shards, one operator's cluster of boxes |

## Clusters and federated meshes

The design target is a **shard mesh** across an operator's many boxes, and past
the cluster, **other operators' nodes joining the same mesh**: federated
compute, paid per-seat or per-hour through the non-custodial `PaymentRail`
(see [Payments](payments.md)). Capacity isn't rented from Magnetite; it's
contributed by whoever chooses to run a node.

> **Status — built, proven on a LAN, unproven on the internet.**
> Multi-shard hosting on a *single* box is real, tested, and deterministic.
> **Cross-node handoff over the network is now built and tested:**
> `NetworkHandoffTransport` opens an Ed25519-authenticated TCP channel to the
> node that should own the target shard (both sides prove control of their node
> keypair; the caller pins the key it expects, so reaching the right *address*
> is not enough) and runs a **two-phase, epoch-fenced migration** — the target
> validates and stages the state and acks it, and only after a verified
> commit-ack does the source release authority. Every partial failure — ack
> timeout, rejection, dropped connection, target crash — resolves to *the
> source still owns the shard*, with its state intact; duplicate and replayed
> handoffs are refused by a monotonic per-shard epoch. Determinism is asserted
> across the migration boundary: a shard that moved produces byte-identical
> results to one that never did. `SpreadScheduler` places shards across nodes
> by capacity, so a bigger box takes more shards.
>
> **A cluster now configures itself.** Routes used to be hand-registered. A node
> can now derive them from the *signed* ads already flowing through discovery:
> `RouteDirectory::observe` turns "this key says it is at this address" into a
> route with the key **pinned**. Discovery supplies addresses only — see
> "Who is allowed to receive a shard" below.
>
> **A player's session now follows the shard — through the actual socket.**
> When a shard commits a migration from A to B, A hands each affected client a
> `SignedRedirect` — the target's address *and* pinned node key, the shard, the
> new epoch, an expiry, and a short-lived single-use `FollowToken` — signed by
> A's node key. The client reconnects to B, aborts unless B proves it holds the
> pinned key, and presents the token. It is a **redirect, not a proxy**: the
> source does not stay in the path, which is the whole point of moving the shard.
>
> This is wired end to end, not just as a mechanism. Attach a
> `follow::FleetSession` to `GameServerConfig::fleet` and the node's own
> WebSocket listener will: track who is connected on which shard; deliver the
> redirect on the player's live socket the moment a migration commits, then close
> that connection; and run any incoming `ClientNet::Follow` through
> `FollowAdmission::admit` before attaching the player — under the player id the
> redirect was minted for, so the session is continuous rather than a fresh join.
> `magnetite-runtime/tests/session_follow.rs` proves it over real sockets between
> two real nodes, along with the refusals: a forged redirect, an expired one, one
> retargeted at another player, one from a non-member node, and a replayed one.
> A failed migration is proven to deliver **nothing**.
>
> **The client verifies.** `magnetite-web-client/src/follow.js` checks the
> redirect's issuer signature (Ed25519 via WebCrypto — no hand-rolled curve
> arithmetic, no added dependency) against the node key the session already
> pinned, refuses an expired one, and pins `target_key` on the new connection: it
> asks the far side to sign a fresh nonce and aborts unless the key matches and
> the signature verifies. Where WebCrypto cannot do Ed25519, the follow is
> **refused** — "cannot check" is never treated as "checks out". A client that
> blindly followed a redirect could be walked onto an attacker's node, which is
> the entire threat this protocol exists to stop.
>
> **A configured cluster now balances itself.** The pieces above could move a
> shard safely and say who was allowed to receive one, but nothing ever *decided*
> to move anything — three configured nodes would sit there with every shard on
> whichever one created it. `rebalance::Rebalancer` closes that: every pass it
> asks its member peers what they hold and how big they are, runs
> `SpreadScheduler` over the visible cluster, and sheds whatever it is holding
> beyond its own capacity share — through the same two-phase handoff, unchanged.
> It ships with four brakes on by default (deadband, per-shard cooldown, a cap on
> concurrent migrations, and exponential per-peer backoff), because a reconciler
> that reacts to every measurement thrashes. **A converged cluster issues zero
> migrations**, and `convergence_then_zero_migrations` asserts exactly that over
> real sockets. See "Running a self-balancing cluster" below.
>
> **A node that dies loses its shards' state — permanently.** There is no state
> replication. The rebalancer detects the loss, stops routing work to the dead
> node, and reports what was lost **as a loss**; it does not quietly start empty
> replacement shards, which would look like recovery in every log line while the
> players in them lost their sessions.
>
> **What is NOT proven:** all of this is tested over real sockets between
> processes on one machine and on a LAN. It has **not** been run across the
> public internet, and there is **no NAT traversal, no hole punching, and no
> relay** — nodes must be able to reach each other directly (same LAN, a VPN, or
> public IPs with the handoff port open). WAN latency, packet loss, asymmetric
> partitions, and clock skew at internet scale are untested. Treat fleets as a
> single-datacenter / single-network capability today. (A NAT-traversing
> transport could later be offered behind the same `HandoffTransport` seam;
> none is implemented, and cross-node handoff will not be made to depend on an
> optional protocol.)

## Discovery is a phonebook, not a gatekeeper

Nodes self-advertise (`Discovery::announce`) instead of polling a central
`runtime_instances` table for provisioning work. The default `TrackerDiscovery`
is a dumb, swappable HTTP tracker in the BitTorrent sense — anyone can run one,
and redundancy comes from running more than one, not from Magnetite operating
a single blessed registry. `LanDiscovery` (mDNS) covers the local-network
case with zero external dependency at all.

## Who is allowed to receive a shard

Discovery is an **open phonebook**: anyone can announce, and a well-formed,
correctly-signed ad from a stranger is a perfectly normal thing to see. So an ad
must never be treated as permission to hold your world's state — otherwise
anyone who volunteers gets handed your shards.

The rule Magnetite enforces:

> Discovery may supply an **address**. Only the operator confers **membership**,
> and membership is keyed on the node's **public key**.

`ClusterMembership` is that operator-authorized key set. It is **deny by
default** — an empty membership authorizes nobody, so a missing or half-applied
config hands shards to *no one* rather than to *anyone*. It is enforced in three
places, each of which fails closed:

- `RouteDirectory::observe` refuses an ad whose signature does not verify, whose
  lease has lapsed, or whose `node_key` is not a member — in that order — and
  learns nothing at all from a rejected ad.
- `NetworkHandoffTransport::with_membership` re-checks membership **at migration
  time**, before a byte of state leaves the box. A hand-registered route to a
  non-member is refused just the same.
- The `FleetNode` inbound allowlist (`ClusterMembership::allowlist`) gates the
  other direction, so the same operator decision guards both doors.

Key pinning is unchanged and still load-bearing: the pinned key comes from the
*signed ad*, never from the address, and the handshake aborts if the far side
presents anything else. Announcing that you host a game therefore never makes
you eligible to receive shards of a world you were not admitted to. Revocation
takes effect on the next lookup — you do not wait for a lease to lapse.

The same membership set gates session-follow: node B admits a redirected player
only if the `FollowToken` was issued by a **member**, names B as its target,
verifies, is unexpired and unredeemed, and matches the player, the shard, and
the epoch B *actually owns right now*. A token for player X will not admit
player Y; a token for shard S will not admit to shard T; a redirect from a
superseded migration is refused by the same epoch fence that governs handoff.
Redirects are minted only after a verified commit-ack, so a failed or
rolled-back migration never sends anyone anywhere.

### What session-follow does not do

Being plain about the edges, because each of these is a real limit:

- **A redirect is a bearer credential.** Anyone who can read a player's redirect
  before it is redeemed can redeem it in their place, once, within its ~30s
  window. It is single-use, epoch-fenced and short-lived, which bounds the
  damage — it does not eliminate it. Run players over `wss://`.
- **The node-identity proof authenticates the key, not the channel.** A node
  answering `ClientNet::Hello` proves it holds the secret half of its node key.
  It does not bind that proof to the transport, so on plaintext `ws://` a relay
  in the middle is not defeated by it. TLS is still doing real work.
- **No NAT traversal.** Unchanged and unqualified: the redirect's address must be
  directly reachable by the client, exactly as the handoff port must be
  reachable by peer nodes.
- **A node with no configured peers does not enable it.** That is not an
  omission, it is the deny-by-default rule: no membership means no handoff
  listener, no migration transport, and `fleet: None` — a plain single-box node,
  exactly as before. Session-follow turns on only when an operator names the
  peer keys they trust (see "Running a two-node cluster" below).

## Running one

```bash
# build your game to wasm (see Getting started)
magnetite build

# run it locally with zero backend
magnetite dev

# put it on hardware you own — LAN discovery, nothing external required
magnetite node --wasm path/to/game.wasm --host 0.0.0.0 --port 9000

# ...and additionally announce to a tracker
TRACKER_URL=https://tracker.example.org magnetite node --wasm path/to/game.wasm
```

`magnetite node` builds (or loads) the module, content-addresses it with
BLAKE3, **verifies the hash before executing it**, measures this box, and
self-advertises. The tracker is opt-in: with no `TRACKER_URL`, `LanDiscovery`
(mDNS) is the default and no external service is involved. An unreachable
tracker is treated as a lost hint, not a failed boot; the node renews its lease
on a heartbeat and retracts its ad on exit.

There is no cloud account to create and no capacity to request — the box you
run it on *is* the capacity.

## Running players over `wss://`

**The node's listener is plaintext `ws://` only.** `magnetite-runtime/src/server.rs`
binds a bare `TcpListener` and calls `tokio_tungstenite::accept_async` on the raw
socket — there is no TLS anywhere in the accept path, and that is a real limit
rather than an omission: a page served over `https://` cannot open a `ws://`
socket at all. Mixed-content blocking has no operator-side workaround, so
**every** browser game — a three.js client, Godot's HTML5 export, a `wibbly`
page — hits this the moment it is not `http://localhost` or `127.0.0.1`
(those two count as a secure context, which is why local development needs
nothing extra). This is the R1 blocker `ALIGNMENT.md` §2 names, and it is
accurate: nothing here builds TLS into the node.

Node-to-node traffic (shard handoff, cluster membership) is a different case
and does **not** need this: `magnetite-runtime/src/follow.rs` already says "run
behind TLS" for that path, and FlowStock's shipped answer — a trusted network
(LAN, VPN/overlay, or a tunnel) rather than in-process TLS — applies there
unchanged, because both ends are Magnetite's own binary. Browsers are the part
that cannot be told to trust a network; they enforce the secure-context rule
unconditionally.

### The options, and which one this ships

Three routes exist for a player-facing `wss://` endpoint:

1. **A reverse proxy terminating TLS in front of the node** (this section, verified below).
2. **A tunnel** (ngrok, cloudflared, or the suite's own
   [Ephor](https://github.com/vul-os/ephor)) — reachability without renting or
   configuring anything, at the cost of a content-visible L7 hop and, on free
   tiers, a URL that changes on restart.
3. **rustls + ACME built into the node binary** — the most self-contained
   story (no separate proxy process) but the largest change, and it needs a
   real internet-routable domain to exercise the ACME HTTP-01/TLS-ALPN-01
   challenge. **Not built here, and explicitly unexercised** — there is no
   domain available in this environment to prove an ACME issuance actually
   completes, and claiming the path works without running it would be exactly
   the kind of unverified status claim this project has repeatedly had to
   retract. Left as future work if an operator wants one process instead of two.

This adopts route 1, matching the pattern already shipped for the same problem
elsewhere in the family rather than inventing a third recipe:
[flowstock's `docs/CLOUD-NODE.md`](https://github.com/vul-os/flowstock/blob/main/docs/CLOUD-NODE.md)
and [pango's `docs/CLOUD-NODE.md`](https://github.com/vul-os/pango/blob/main/docs/CLOUD-NODE.md)
both terminate TLS with a proxy in front of a loopback-bound process, document
a tunnel as the zero-infrastructure alternative, and are explicit that the app
itself does not speak TLS. The recipe below copies that shape, not any code —
Magnetite gains no new dependency on either sibling.

### The recipe

Bind the node to loopback so the proxy is the only path in:

```bash
magnetite node --wasm game.wasm --host 127.0.0.1 --port 9000
```

Then terminate TLS with Caddy — the shortest honest version, same as the
siblings' recipe — using the file committed at
[`magnetite-runtime/deploy/Caddyfile.example`](../magnetite-runtime/deploy/Caddyfile.example):

```caddyfile
your-node.example.com {
	reverse_proxy 127.0.0.1:9000
}
```

```bash
caddy run --config Caddyfile.example
```

Caddy obtains and renews a Let's Encrypt certificate for the real domain on its
own. Players connect to `wss://your-node.example.com` — never to port `9000`
directly, which a firewall should not expose:

| Port | Who needs it |
|---|---|
| 443 | Players, via the proxy. |
| 80 | ACME only. |
| 9000 | **Nobody.** Do not open it if the proxy is on the same host. |

nginx or HAProxy work the same way; bring your own certificate with those
instead of Caddy's automatic one.

### What was actually verified here (2026-07-30)

No domain is reachable from this machine, so the ACME path above is
documented, not exercised. What **was** run, end to end, on this host:

1. `magnetite-serve --host 127.0.0.1 --port 9000` (the NopGame smoke-test
   binary — no `.wasm` required) — plaintext, exactly as shipped today.
2. Caddy in front of it with `tls internal` instead of a real domain — Caddy's
   own local CA mints a certificate for `localhost`, which is enough to prove
   the `wss://` code path without owning a domain (see the commented block at
   the bottom of `Caddyfile.example`).
3. A Python client (`websockets` 16.0) opened `wss://localhost:9443/`, with
   the TLS chain verified against Caddy's local root — not `--insecure`.

Result: the TLS handshake completed, the WebSocket upgrade completed, and the
node's own connection-tracking log lines fired for a real player:

```
WSS HANDSHAKE OK — TLS verified, WebSocket upgrade complete
negotiated: 1
received first server frame, len= 146
```

```
2026-07-30T17:48:01.904154Z  INFO magnetite_runtime::server: player connected peer_addr=127.0.0.1:54581 player_id=Player(1)
2026-07-30T17:48:01.904893Z  INFO magnetite_runtime::server: player disconnected player_id=Player(1)
```

The 146-byte frame is the real `ServerNet::Welcome` sent by
`magnetite-runtime/src/server.rs`, not a proxy-level stand-in — it only exists
if the TLS layer, the WS upgrade, and the node's own accept path all worked.
(A first attempt with an explicit `header_up Upgrade`/`Connection` override in
the Caddyfile produced a 502 — Caddy's automatic WebSocket detection handles
this correctly on its own and the override interfered with it; the recipe
above deliberately does not set those headers.)

### What this does and does not prove

- **`wss://` through a proxy is a genuine, verified unblock for R1** — a
  browser on an `https://` page can reach a magnetite node once a
  TLS-terminating proxy sits in front of it, which is exactly what was
  missing.
- **The node itself still speaks plaintext `ws://` on its own listener.**
  Nothing in `magnetite-runtime/src/server.rs` changed. TLS terminates one hop
  before the node, not in it — say it that way, not "the node supports TLS".
- **The ACME path is unexercised.** The verification above used Caddy's local
  CA, not a real Let's Encrypt issuance against a public domain, because no
  such domain is reachable from this environment. Treat that as untested until
  someone runs it against a real DNS name.
- **A tunnel is optional and third-party, never a default.** If you use one,
  the tunnel operator is a content-visible L7 hop in the path — the same
  caveat pango's `CLOUD-NODE.md` §5 states for its own tunnel option. Ephor is
  one tunnel provider among several here, not a dependency: nothing in
  `magnetite-runtime`, `magnetite-cli` or their build files references it, and
  it must stay that way (see `wede`'s hard, undeclared Ephor dependency for
  the mistake this is avoiding).
- **Still open:** WAN validation of the rest of the stack (shard migration,
  membership, session-follow, the attested wire) across two real internet
  hosts, and a node deploy recipe distinct from the legacy central backend's —
  both separate items already tracked in `ALIGNMENT.md` §2.

## Node identity is a key file, not an address

On first run a node generates an Ed25519 keypair and writes the seed to
`~/.magnetite/node.key` (or `$MAGNETITE_HOME/node.key`, or `--node-key-file`),
owner-readable only (`0600`). Every restart reuses it, so **a node's identity is
stable across restarts and across a change of bind address** — which matters
because peers pin that key in their membership lists and a tracker binds your
listing slot to it. The public key is printed at startup:

```text
  Node pubkey      : ceba6d97cabf9324052d87ff4281c39d3f12db49f76b26aaf1ef7ab81f4636d3
  Node key         : /home/you/.magnetite/node.key (stable)
```

That hex string is what you paste into another node's membership list. Back the
key file up and keep it secret: whoever holds it *is* this node. `MAGNETITE_NODE_SEED`
(32-byte hex) still overrides the file for ephemeral/containerised setups; if it
is set but malformed the node **refuses to start** rather than quietly booting
under a different identity. Only if no key file location can be determined at all
(no `HOME`, no flag) does the node fall back to deriving a key from its bind
address — it says so at startup, and in that mode the identity is *not* stable.

## Running a self-balancing cluster

Two boxes, `10.0.0.11` and `10.0.0.12`, on the same LAN or VPN. Both must be able
to reach each other's **handoff port** directly.

**1. Start each node once to mint and print its key.**

```bash
# on 10.0.0.11
magnetite node --wasm game.wasm --host 0.0.0.0 --port 9000
#   Node pubkey      : ceba6d97…36d3      ← key of node A
# Ctrl-C

# on 10.0.0.12
magnetite node --wasm game.wasm --host 0.0.0.0 --port 9000
#   Node pubkey      : fff965a4…e584      ← key of node B
# Ctrl-C
```

**2. Put each node's key *and address* in the other's membership list.**

```bash
# on 10.0.0.11 — authorize B, and say where B answers
magnetite node --wasm game.wasm --host 0.0.0.0 --port 9000 \
  --handoff-addr 0.0.0.0:9001 \
  --cluster-peer fff965a4de11f1869c9ac096d9d3bae02b8aa75614c05ed8c9fa1210f95ae584@10.0.0.12:9001

# on 10.0.0.12 — authorize A
magnetite node --wasm game.wasm --host 0.0.0.0 --port 9000 \
  --handoff-addr 0.0.0.0:9001 \
  --cluster-peer ceba6d97cabf9324052d87ff4281c39d3f12db49f76b26aaf1ef7ab81f4636d3@10.0.0.11:9001
```

The `@host:port` suffix is new and it is what makes the cluster *self-balancing*.
A bare key still works and still fully authorizes the peer — but without an
address there is nowhere to send work, so nothing will be placed there.

**The key is the authorization; the address is only a location.** If the address
is wrong, or someone else has taken it, the handoff handshake aborts because the
far side cannot prove control of the pinned key. A stolen address gets nothing.

Both must serve the **same game.wasm**: the game id is the BLAKE3 hash of the
module, so a mismatched binary is a different game and the nodes will not find
each other in discovery.

Each prints its cluster state:

```text
  Cluster          : 1 authorized peer key(s)
  Handoff listener : 0.0.0.0:9001 (node-to-node only, mutually authenticated)
  Session follow   : ON — players are redirected when a shard migrates
  Reachability     : peers must reach 0.0.0.0:9001 DIRECTLY — no NAT traversal,
                     no hole punching, no relay (same LAN / VPN / public IP)
  Routable peers   : 1 of 1 have an address
  Rebalancer       : ON — every 30s, capacity-aware, deadband 1 shard(s),
                     at most 2 migration(s) per pass
  Node death       : LOSES that node's shard state — there is NO state
                     replication; losses are reported, never 'recovered'
```

**3. Watch shards distribute by capacity.** Every 30 seconds each node asks its
peers what they hold and how big they are, runs `SpreadScheduler` over the whole
visible cluster, and hands over anything it is holding beyond its own share:

```text
  rebalance: shard 3 -> fff965a4…e584 (epoch 1)
  rebalance: shard 4 -> fff965a4…e584 (epoch 2)
```

The share tracks the `Emergent cap` line in each node's banner — measured
hardware, not a number you set. An 8-core box and a 2-core box do not get four
shards each.

Then it goes quiet. **A converged cluster issues zero migrations**, forever, and
that is asserted directly by the `convergence_then_zero_migrations` test: it
drives a three-node cluster to a fixed point over real sockets and then requires
twenty further passes to move nothing at all.

**4. Add a third node and watch it take share.** Start `10.0.0.13`, add its
`key@addr` to A and B (and theirs to it), and within a pass or two the existing
nodes shed toward it. Nothing else changes; the cluster re-converges and goes
quiet again.

**5. Watch a player follow a migration.** Connect a client to A
(`ws://10.0.0.11:9000`). When the shard that player is on migrates to B, A hands
the client a `SignedRedirect` on its live socket and closes it; the client
reconnects to B, requires B to prove the pinned key, presents its single-use
`FollowToken`, and is re-attached **under the same player id** — one continuous
session, no re-join.

### The brakes, and why they are not optional

A rebalancer that reacts to every measurement is worse than none: shards
ping-pong between nodes, every move costs the players in it a reconnect, and the
cluster spends its capacity migrating instead of simulating. Four brakes ship on
by default.

| Brake | Default | What it prevents |
|---|---|---|
| **Deadband** | 1 shard | Moving for an imbalance smaller than a greedy bin-pack's own tie-breaking noise. |
| **Cooldown** | 120s per shard | A shard that just moved being sent straight back by the next, slightly staler, view. |
| **Concurrency cap** | 2 per pass | Draining a node all at once, and the reconnect storm that follows. |
| **Peer backoff** | 5s, doubling to 300s | Hammering a node that is down. A backed-off peer is not contacted at all. |

Two design choices do most of the work:

- **Placement ignores current ownership.** The desired layout is a pure function
  of *which shards exist* and *which nodes exist*, so moving a shard never
  changes where anything is supposed to be. Each move strictly reduces total
  imbalance, which is a non-negative integer — so the loop must terminate.
- **We balance counts, not identities.** If a node holds three shards and the
  scheduler wants it to hold three *different* ones, that is a swap with no
  measurable benefit and two migrations' worth of cost. Nothing moves.

### When a node dies

**Its shards' state dies with it. This is not recoverable and nothing in
Magnetite pretends otherwise.** There is no state replication: a shard's world
lives in one node's memory, and if that node is gone, so is it.

What actually happens:

- The surviving nodes' probes fail, the peer enters exponential backoff, and it
  stops receiving new work immediately.
- Shards it was last seen holding, which no surviving node now reports, are
  reported as **losses**, once, in full:

  ```text
    rebalance: shard 6 STATE LOST: node fff965a4…e584 held it and it stopped
    answering: connect to 10.0.0.12:9001: Connection refused. There is no state
    replication in Magnetite, so this shard's in-memory world is gone — a
    replacement shard would be a NEW world, not a recovered one
    rebalance: shard 6 will NOT be restarted automatically; starting one would
    create a NEW world, not restore the old one
  ```

- The remaining capacity re-balances across the surviving nodes. That is
  **re-placement of capacity, not recovery of state**.

Nothing restarts those shards for you, deliberately. An empty shard with the same
id would look like a successful recovery in every log line and metric while every
player who was in it silently lost their session. If you want a fresh shard, ask
for one knowing it is fresh.

Players on a dead node also do not get a redirect — redirects are minted only by
a *successful* migration, and a node that died did not perform one. They see a
dropped connection and must re-join.

> **TODO (not attempted here): state replication.** The honest fix is replicating
> shard state to a warm standby, or checkpointing it to the blob-store seam, so a
> node's death costs a rollback rather than the whole shard. That needs quorum
> rules, a story for split-brain against the epoch fence, and a checkpoint
> cadence — a much larger design than this loop.

### Flags and environment

| Flag | Env | Meaning |
|---|---|---|
| `--cluster-peer <HEX[@ADDR]>` (repeatable) | `MAGNETITE_CLUSTER_PEERS` (comma/space separated) | Authorized peer node public key, 64 hex chars, optionally `@host:port` for its handoff listener |
| `--cluster-peers-file <PATH>` | `MAGNETITE_CLUSTER_PEERS_FILE` | One entry per line, `#` comments — for lists longer than a couple of nodes |
| `--handoff-addr <ADDR>` | `MAGNETITE_HANDOFF_ADDR` | Node-to-node listener, separate from the game port. Defaults to `<host>:<port+1>` |
| `--node-key-file <PATH>` | `MAGNETITE_NODE_KEY_FILE` | Persisted node keypair. Default `$MAGNETITE_HOME/node.key`, else `~/.magnetite/node.key` |
| `--no-rebalance` | — | Turn the reconciler off; placement becomes entirely manual |
| `--rebalance-interval <SECS>` | — | Seconds between passes (default 30) |
| `--rebalance-deadband <N>` | — | Shards of slack before anything moves (default 1) |
| `--rebalance-max-in-flight <N>` | — | Migrations per pass (default 2) |
| — | `MAGNETITE_NODE_SEED` | 32-byte hex seed; overrides the key file |

The rebalancer is **on by default whenever at least one peer has an address**,
because a cluster that has been told who its members are and how to reach them,
and then refuses to distribute work, is the bug this loop exists to fix. It is
off when no peer is routable — which is exactly the deny-by-default case — and
`--no-rebalance` turns it off explicitly.

Sources merge and de-duplicate on the **key**. A malformed entry is a **hard
error naming the offending entry** (and, for a file, its line number) — never a
silently dropped peer, because a membership list you cannot trust to be complete
is worse than one that fails to load. An unreadable peers file is an error too,
not an empty allowlist. `key@` with an empty address is an error as well: it is
almost always a truncated config line, and treating it as "member with no
address" would quietly remove that node from every placement decision.

### What this walkthrough does and does not prove

- **No peers configured means no cluster.** Not "trust anyone" — the handoff
  listener is not even bound, so there is nothing for a stranger to talk to.
  Membership is deny-by-default all the way down, and the same explicit key set
  gates the inbound allowlist, the outbound transport, the route directory, and
  every placement the rebalancer proposes. An address in a config file authorizes
  nothing on its own.
- **A failed migration always leaves the source owning the shard**, with its
  state intact — including a migration the rebalancer started. There is no
  partial handoff and no window in which nobody owns a shard.
- **Still no NAT traversal, no hole punching, no relay.** Nodes must be directly
  reachable: same LAN, a VPN, or a public IP with the handoff port open.
  Operation across the public internet is **untested** — treat a fleet as a
  single-network capability today. The rebalancer does not change this; it cannot
  route around an unroutable node, it can only back off from it and place work
  elsewhere.
- **Node death loses that node's shard state.** No replication, no resurrection,
  and losses are reported as losses. See above.
- **A redirect is a bearer credential** within its ~30s window: whoever reads it
  before it is redeemed can redeem it once. Run players over `wss://`.
- **The node-identity proof authenticates the key, not the channel.** It does not
  bind to the transport, so TLS is still doing real work.
- **Peer capacity is self-reported.** A node's status answer is its own claim,
  made over an authenticated channel. A member that lies about its size can only
  attract or repel shards it was already authorized to hold — every actual
  handoff to it is still membership-checked, key-pinned, two-phase and
  epoch-fenced — but placement is not defence against a dishonest *member*.
