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
> **A player's session now follows the shard.** When a shard commits a migration
> from A to B, A hands each affected client a `SignedRedirect` — the target's
> address *and* pinned node key, the shard, the new epoch, an expiry, and a
> short-lived single-use `FollowToken` — signed by A's node key, which the client
> has already authenticated. The client reconnects to B, aborts unless B presents
> the pinned key, and presents the token. It is a **redirect, not a proxy**: the
> source does not stay in the path, which is the whole point of moving the shard.
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
