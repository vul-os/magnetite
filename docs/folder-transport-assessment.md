# Folder transport assessment (A23)

**Verdict, up front:** FlowStock's folder sync is real, shipped, tested Go code —
not a documented pattern with nothing behind it. But it solves a harder problem
than magnetite has for packages, and a *different* problem than magnetite has
for replay logs. The reusable thing is not the code (wrong language, wrong data
model, and owner directive #1 in `FANOUT-LOOP-STATE.md` forbids a cross-product
dependency anyway) — it is the underlying shape. That shape fits magnetite's
**immutable, content-addressed packages** almost for free, using tooling that
already exists (rsync/Syncthing/Dropbox/a USB stick) plus a naming convention,
no new magnetite code. It fits a **live, still-growing replay log** structurally
(single writer, tailed incrementally) but magnetite would have to hand-write
that from scratch, and it would need to add an authenticity layer FlowStock's
own bare folder path does not have either.

## What FlowStock's folder sync actually is

Real, merged, tested code — read directly, not inferred from the doc:

- `flowstock/backend/internal/sync/folder.go` (198 lines): `Engine.FolderSync`
  exports this node's own newly-authored ops to `ops-<node_id>.jsonl` in a
  shared directory (`exportOwn`), then imports every *other* node's
  `ops-*.jsonl` file through the same idempotent `store.ApplyOps` network sync
  uses (`importOthers` → `importFile`).
- `flowstock/backend/internal/sync/folder_test.go`:
  `TestFolderSyncTwoNodesViaSharedDir` proves two nodes converge through a
  `t.TempDir()` standing in for a synced folder, with **no HTTP involved at
  all** — including a late-joining third node that replays full history from
  the files alone.
- `flowstock/e2e/folder-sync.spec.js` exercises it through the UI
  (`Settings → Sync → Sync folder`).
- `flowstock/docs/SYNC.md` "Folder sync" (§150–191) documents exactly this
  code, plus a USB/sneakernet workflow (§173–190) built on the same mechanism.

So: real transport, with code and tests behind every claim in the doc. The
mechanism, precisely:

1. **Single writer per file.** Each node writes only its own file
   (`ops-<node_id>.jsonl`), so the file-sync tool (Dropbox, Syncthing, rsync,
   a USB copy) never has two writers on one file — nothing to merge, ever, at
   the file-sync layer.
2. **Append-only, whole-line consumption.** A reader tracks a byte offset per
   file (`folder_import_off:<name>` in settings) and only consumes complete
   lines, so a file mid-write-by-the-sync-tool is read safely up to its last
   flushed line (`folder.go:174-185`).
3. **Idempotent apply.** Every imported line goes through the same
   `store.ApplyOps` the network path uses — replaying the same op twice, in
   any order, from any carrier, is a no-op.
4. **Files are transport, never truth.** The database is authoritative; the
   files are a durable, replayable log a fresh node can rebuild entirely from
   (`SYNC.md:166-168`).

The whole design exists to solve **leaderless CRDT replication among multiple
writers with no shared clock and no live connection** — every node both
originates new facts (op = "JHB sold 3 of SKU X") and needs everyone else's.
That is a materially harder problem than distributing an artifact one party
published and everyone else only ever reads.

## Against magnetite's packages: fits almost for free, no new machinery needed

`magnetite-seams/src/package.rs`'s `Package`/`PackageManifest` is already:

- **content-addressed** — `PackageManifest::root` is a hash over the sorted
  `path → hash` file list (`package.rs` module docs, property 1);
- **signed** — canonical CBOR bytes, developer-signed, `MANIFEST_DOMAIN`
  (property 2);
- **immutable** — a `Package::id` commits to price, split, determinism class
  and developer key; there is no notion of editing a published package, only
  publishing a new one;
- as of today, **chunk-verifiable** — `magnetite-seams/src/chunktree.rs`
  (A24, landed `70cbee9`) gives any 1 MiB slice of a package file a Merkle
  proof against the manifest root, independent of the rest of the file.

None of FlowStock's actual machinery is needed for this, and here is the
specific reason why: **FlowStock's per-writer-file rule and idempotent-apply
step exist to solve a multi-writer merge problem that content-addressed
immutable data structurally does not have.** A package has exactly one
author (the developer who signed it) and the content itself is the identity —
two copies of the same package are either byte-identical or one of them is
corrupt, and the manifest's per-file hashes (now backstopped by the chunk
tree) already detect corruption without any op-log, HLC, or "which write
wins" logic. Any file-sync tool already replicates files bit-for-bit; that is
the entire job. Dropbox, Syncthing, rsync and a USB stick already do this for
free, today, with zero magnetite code, provided packages are laid out as
files a sync tool can carry.

**What magnetite would actually need to add — small, and not a transport:**

- A content-addressed naming/layout convention for published packages (e.g.
  a directory per `Package::id` or manifest root) so a sync tool has stable
  filenames to reconcile and a consumer has a filename to look up.
- A read path that verifies the manifest signature + per-file hashes (already
  exists — `PackageManifest::validate`, called on load) before trusting
  anything that arrived via a folder rather than a direct fetch. This is
  already how the format is designed to be consumed; a folder is just another
  place bytes can arrive from.
- To actually cash in the chunk tree for **resumable** transfer (the second
  half of A23's insight — "content-addressed packages need no live socket"
  reads better as "…and a partially-copied file can be verified and completed
  without starting over"), something has to range-read a package file, check
  the read chunk against `chunktree::verify_chunk`, and re-request/re-copy
  only the missing chunks. **This does not exist yet.** `chunktree.rs`'s own
  doc comment says plainly it is "not wired into `magnetite-web-host`'s
  serving path" — the primitive landed (A24), the range-aware consumer did
  not. So today, a folder-sync of a magnetite package is verifiable (hash
  mismatch on a corrupt/incomplete file is caught) but not yet resumable at
  sub-file granularity — an interrupted copy has to be verified and, on
  failure, redone whole, until something calls `ChunkTree::prove`/
  `verify_chunk` on the receiving end. Recording this as the honest gap
  rather than implying A24 already closed it.

**Conclusion for packages:** adopt the *pattern*, write no transport code.
Point any existing sync tool at a directory of published `.mag` packages laid
out by content address; the manifest's signature and per-file hashes (and,
once wired, the chunk tree) are the only "protocol" needed. This is a
materially smaller lift than what FlowStock built, precisely because
magnetite's data shape is simpler than FlowStock's.

## Against magnetite's replay logs: right shape, no existing implementation to take

A replay log (`magnetite_sdk::authority::ReplayLog`,
`magnetite-anticheat/src/replay_verifier.rs` module docs) is **not**
package-shaped. While a match is running it is genuinely append-only and
growing — one entry per tick, containing every player's input and the
authoritative state hash, recorded by `ReplayLog::record` tick by tick. That
is a much closer match to FlowStock's op-log shape (one authoritative writer
producing an ordered, ever-growing stream) than to a package's shape
(one static, complete, immutable blob).

Where the current implementation actually sits, checked in code rather than
assumed: `backend/src/api/replays.rs` stores a **finished** `ReplayLog` as one
JSON blob in a Postgres row (`POST /replays`, `ReplayRow.replay_json: Value`),
fetched whole (`GET /replays/:id`). There is no signing and no content
address anywhere in that path today — a replay is identified by a Postgres
`Uuid`, not a hash, and nothing hashes or signs the JSON blob before or after
storage.

Two distinct distribution questions follow from the two different states a
replay log can be in:

**A *finished* replay log (match over, log immutable from here on)** is
exactly package-shaped once it is content-addressed and signed — which it is
not, today. Add a whole-log BLAKE3 hash and a signature (from whichever key
recorded the match — the match host, or the authoritative server), and the
same conclusion as packages applies verbatim: a folder transport suffices,
because integrity and authenticity live in the content, not the channel. This
is small, independently worthwhile work (spectators and anti-cheat tooling
both want a replay they can verify without trusting whoever handed it to
them), and it is **not currently done** — recording it as a real, open
prerequisite rather than assuming it away.

**A *live, still-recording* replay log** (the case a folder transport would
actually be interesting for — e.g. near-real-time spectating off a synced
folder instead of a socket) is where FlowStock's *shape* — one writer, one
file, tailed by byte offset — is the right model. But two things stop this
from being "already solved, go adopt it":

1. **No dependency is possible.** `folder.go` is Go, and is inseparable from
   FlowStock's own `store.Op` (a generic CRDT operation with an HLC
   timestamp) and `store.ApplyOps` (FlowStock's specific merge semantics —
   last-writer-wins for catalog rows, union for ledgers). None of that
   applies to a replay tick record, and per owner directive #1, magnetite may
   not depend on FlowStock's code or on kotva outside of crates.io. Nothing
   at kotva (`crates/kotva-sync`, checked: `body.rs`, `cose.rs`, `crdt.rs`,
   `detcbor.rs`, `recon.rs`, `snapshot.rs`, `state.rs`, `wire.rs` — no folder
   transport in any of them) offers this either, so there is nothing to pull
   from crates.io. Any implementation here is magnetite's to write, informed
   by FlowStock's design, not copied from it.
2. **FlowStock's bare folder path itself has no per-line authenticity, and
   magnetite would need to add exactly that.** `opsMsg`'s batch signature
   (`sync.go:92-118`) — the mechanism that stops FlowStock's *network* sync
   path from trusting an unsigned batch — is a network-transport concept and
   is **not part of `folder.go` at all**: `exportOwn` writes plain
   `json.Marshal(op)` lines with no signature field, and `importFile` applies
   whatever it reads with no signature check. FlowStock accepts that trade
   because (a) `ApplyOps`' idempotency and the `org_id` check are enough for
   its own threat model on a folder the whole workspace already trusts by
   construction (their own shared Dropbox/USB), and (b) the harder guarantee
   (`-tags dmtap`'s per-op COSE_Sign1 envelope, `SYNC.md:213-236`) is a
   different code path layered on top, not something folder sync itself
   provides. A shared folder for a live magnetite replay would need to carry
   its own answer to "who is allowed to have written this tick," because a
   synced directory is not an authenticated channel the way FlowStock's own
   mutual-Ed25519 HTTP transport is — the folder path trades transport
   authentication for trusting whoever can write to the directory, which
   FlowStock's design accepts for its own workspace-trust model and magnetite
   would have to decide for itself, not inherit for free.

**Conclusion for replay logs:** folder sync suits the *finished* shape today
(once magnetite adds the hash+signature it currently lacks — same argument as
packages) and suits the *live* shape only as a design pattern to hand-copy,
not as adoptable code, and only after magnetite decides how a live tick
stream gets authenticated when the channel itself no longer does that job.

## Answering A23's two sub-questions directly

- **Is it a real transport with code, or a documented pattern?** Real,
  merged, unit-and-e2e-tested Go code (`folder.go` + `folder_test.go` +
  `e2e/folder-sync.spec.js`), matching `SYNC.md`'s description exactly.
- **Does it suit packages, replay logs, both, or neither?** Both, in the
  narrow sense that a shared-folder transport is workable for both shapes —
  but for structurally different reasons and at very different cost:
  packages need almost nothing new (a naming convention plus verification
  code that already exists); finished replay logs need one small addition
  (hash + signature) magnetite doesn't have yet; live replay logs need
  magnetite to design and write its own append-only-file-plus-authenticity
  mechanism, for which FlowStock is a useful reference and zero lines are
  reusable.

## What could not be established

- Whether any consumer (spectator tooling, anti-cheat) actually wants
  **live** replay distribution off a synced folder, as opposed to only ever
  wanting the finished log — could not establish; no such requirement is
  written down anywhere found in this repo's docs. Absent that requirement,
  the live-log half of this assessment is a capability question, not a
  backlog item with a waiting consumer.
- Whether FlowStock's own folder-sync path has been run against a real
  Dropbox/Google Drive/Syncthing client (as opposed to the `t.TempDir()`
  stand-in and the e2e spec) — not checked here; out of scope for this
  assessment, which is about magnetite, not re-auditing FlowStock's own gate.
