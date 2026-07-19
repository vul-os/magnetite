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

An operator who brings many boxes gets a **shard mesh** with cross-node
handoff — the cluster case. Past the cluster, **other operators' nodes can
join the same mesh**: federated compute, paid per-seat or per-hour through the
non-custodial `PaymentRail` (see [Payments](payments.md)). This is the real
"bring any server, it scales to infinity" property — capacity isn't rented
from Magnetite, it's contributed by whoever chooses to run a node, and paid
for wallet-to-wallet between the player, the developer, and the operator.

## Discovery is a phonebook, not a gatekeeper

Nodes self-advertise (`Discovery::announce`) instead of polling a central
`runtime_instances` table for provisioning work. The default `TrackerDiscovery`
is a dumb, swappable HTTP tracker in the BitTorrent sense — anyone can run one,
and redundancy comes from running more than one, not from Magnetite operating
a single blessed registry. `LanDiscovery` (mDNS) covers the local-network
case with zero external dependency at all.

## Running one

```bash
# build your game to wasm (see Getting started)
magnetite build

# run it locally with zero backend
magnetite dev

# put it on hardware you own and join the discovery mesh
magnetite serve --wasm path/to/game.wasm --advertise tracker.example.org
```

There is no cloud account to create and no capacity to request — the box you
run it on *is* the capacity.
