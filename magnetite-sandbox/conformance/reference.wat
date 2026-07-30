;; ===========================================================================
;; reference.wat — a conforming `mag_*` sandbox module written in raw
;; WebAssembly text format.
;;
;; This file exists to demonstrate one claim and nothing more: the `mag_*`
;; sandbox ABI is a plain C-shaped calling convention over linear memory, and a
;; module can satisfy it without any Rust, any SDK, any WASI import, and any
;; language runtime. There is no Rust in this file. There is no C in this file.
;; It is hand-written Wasm.
;;
;; It is NOT a game. It keeps two 64-bit words of state (a tick counter and a
;; rolling FNV-1a hash of everything it has been handed) and emits the three
;; payload shapes the contract requires. Its "rules" are a hash fold; a real
;; module would put its simulation where the fold is.
;;
;; Contract: site/docs/sandbox-abi.md
;;
;; Memory map
;;   0x0000 .. 0x0100   unused
;;   0x0100 .. 0x1000   string literals (data segments below)
;;   0x1000 .. 0x110000 bump arena (reset at the start of every mag_step,
;;                      after the tick's inputs have been consumed)
;; ===========================================================================

(module
  ;; The contract requires linear memory to be exported under the name "memory".
  ;; 17 pages = 1_114_112 bytes. The host's memory cap applies to this.
  (memory (export "memory") 17)

  ;; ---- module state ------------------------------------------------------
  (global $bump (mut i32) (i32.const 0x1000)) ;; bump arena cursor
  (global $tick (mut i64) (i64.const 0))      ;; last tick the host asked us to run
  (global $hash (mut i64) (i64.const 0))      ;; rolling fold of consumed inputs
  (global $seed (mut i64) (i64.const 0))      ;; derived from the MatchConfig JSON

  ;; ---- string literals ---------------------------------------------------
  (data (i32.const 0x100) "{\"rejects\":[],\"state_hash\":")  ;; 27 bytes @ 256
  (data (i32.const 0x120) "{\"tick\":")                       ;;  8 bytes @ 288
  (data (i32.const 0x12c) ",\"hash\":")                       ;;  8 bytes @ 300
  (data (i32.const 0x138) "{\"player\":")                     ;; 10 bytes @ 312
  (data (i32.const 0x144) ",\"tick\":")                       ;;  8 bytes @ 324
  ;; JSON keys looked up by $int_after — searched for, so field order does not
  ;; matter and this module does not depend on serde's emission order.
  (data (i32.const 0x150) "\"tick\":")                        ;;  7 bytes @ 336
  (data (i32.const 0x158) "\"hash\":")                        ;;  7 bytes @ 344

  ;; =========================================================================
  ;; helpers
  ;; =========================================================================

  ;; Bump-allocate `len` bytes, 8-byte aligned. Traps if the arena is full —
  ;; a trap is the contract's prescribed behaviour for an unserviceable
  ;; allocation (never a null return the host would have to guess at).
  (func $alloc (param $len i32) (result i32)
    (local $p i32)
    (local.set $p
      (i32.and (i32.add (global.get $bump) (i32.const 7)) (i32.const -8)))
    (if (i32.gt_u (i32.add (local.get $p) (local.get $len))
                  (i32.const 1114112))
      (then (unreachable)))
    (global.set $bump (i32.add (local.get $p) (local.get $len)))
    (local.get $p))

  ;; Copy `len` bytes src -> dst; returns the cursor just past the copy.
  ;; Deliberately byte-at-a-time: no bulk-memory proposal required.
  (func $puts (param $dst i32) (param $src i32) (param $len i32) (result i32)
    (local $i i32)
    (block $done
      (loop $l
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (i32.store8 (i32.add (local.get $dst) (local.get $i))
                    (i32.load8_u (i32.add (local.get $src) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $l)))
    (i32.add (local.get $dst) (local.get $len)))

  ;; FNV-1a 64 over [ptr, ptr+len).
  (func $fnv (param $h i64) (param $ptr i32) (param $len i32) (result i64)
    (local $i i32)
    (block $done
      (loop $l
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $h
          (i64.xor (local.get $h)
                   (i64.load8_u (i32.add (local.get $ptr) (local.get $i)))))
        (local.set $h (i64.mul (local.get $h) (i64.const 0x100000001b3)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $l)))
    (local.get $h))

  ;; Find `key` (klen bytes at kptr) inside [ptr, ptr+len). Returns the index of
  ;; the first byte after the match, or -1 if absent. Naive scan — the payloads
  ;; are a few hundred bytes and fuel is not scarce at that size.
  (func $find (param $ptr i32) (param $len i32) (param $kptr i32) (param $klen i32) (result i32)
    (local $i i32) (local $j i32) (local $last i32)
    (if (i32.gt_u (local.get $klen) (local.get $len)) (then (return (i32.const -1))))
    (local.set $last (i32.sub (local.get $len) (local.get $klen)))
    (block $done
      (loop $outer
        (br_if $done (i32.gt_u (local.get $i) (local.get $last)))
        (local.set $j (i32.const 0))
        (block $mismatch
          (loop $inner
            (br_if $mismatch
              (i32.ne
                (i32.load8_u (i32.add (local.get $ptr) (i32.add (local.get $i) (local.get $j))))
                (i32.load8_u (i32.add (local.get $kptr) (local.get $j)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))
            (br_if $inner (i32.lt_u (local.get $j) (local.get $klen)))
            ;; full match
            (return (i32.add (local.get $ptr) (i32.add (local.get $i) (local.get $klen))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $outer)))
    (i32.const -1))

  ;; Parse the unsigned decimal integer starting at `p`, stopping at the first
  ;; non-digit or at `end`.
  (func $parse_u64 (param $p i32) (param $end i32) (result i64)
    (local $v i64) (local $b i32)
    (block $done
      (loop $l
        (br_if $done (i32.ge_u (local.get $p) (local.get $end)))
        (local.set $b (i32.load8_u (local.get $p)))
        (br_if $done (i32.lt_u (local.get $b) (i32.const 48)))
        (br_if $done (i32.gt_u (local.get $b) (i32.const 57)))
        (local.set $v
          (i64.add (i64.mul (local.get $v) (i64.const 10))
                   (i64.extend_i32_u (i32.sub (local.get $b) (i32.const 48)))))
        (local.set $p (i32.add (local.get $p) (i32.const 1)))
        (br $l)))
    (local.get $v))

  ;; The unsigned integer that follows `key` in the JSON at [ptr, ptr+len).
  ;; Traps if the key is absent — a payload missing a required field is a
  ;; contract violation, and a silent default is what makes such things
  ;; invisible.
  (func $int_after (param $ptr i32) (param $len i32) (param $kptr i32) (param $klen i32)
                   (result i64)
    (local $at i32)
    (local.set $at
      (call $find (local.get $ptr) (local.get $len) (local.get $kptr) (local.get $klen)))
    (if (i32.eq (local.get $at) (i32.const -1)) (then (unreachable)))
    (call $parse_u64 (local.get $at) (i32.add (local.get $ptr) (local.get $len))))

  ;; Decimal digit count of an unsigned 64-bit value.
  (func $u64_len (param $v i64) (result i32)
    (local $n i32)
    (local.set $n (i32.const 1))
    (block $done
      (loop $l
        (br_if $done (i64.lt_u (local.get $v) (i64.const 10)))
        (local.set $v (i64.div_u (local.get $v) (i64.const 10)))
        (local.set $n (i32.add (local.get $n) (i32.const 1)))
        (br $l)))
    (local.get $n))

  ;; Write `v` as unsigned decimal ASCII at `dst`; returns the cursor past it.
  (func $put_u64 (param $dst i32) (param $v i64) (result i32)
    (local $n i32) (local $i i32)
    (local.set $n (call $u64_len (local.get $v)))
    (local.set $i (local.get $n))
    (block $done
      (loop $l
        (br_if $done (i32.eqz (local.get $i)))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (i32.store8 (i32.add (local.get $dst) (local.get $i))
          (i32.add (i32.const 48)
            (i32.wrap_i64 (i64.rem_u (local.get $v) (i64.const 10)))))
        (local.set $v (i64.div_u (local.get $v) (i64.const 10)))
        (br $l)))
    (i32.add (local.get $dst) (local.get $n)))

  ;; Finish an output buffer: write the 4-byte little-endian payload length at
  ;; `base` (i32.store is little-endian by definition in Wasm, which is exactly
  ;; the byte order the contract specifies) and return `base`.
  (func $finish (param $base i32) (param $end i32) (result i32)
    (i32.store (local.get $base)
      (i32.sub (local.get $end) (i32.add (local.get $base) (i32.const 4))))
    (local.get $base))

  ;; The value reported as `state_hash`: a splitmix64-style finalizer over
  ;; (seed, folded inputs, tick). Pure function of module state — no clock,
  ;; no entropy, no imports.
  (func $state_hash (result i64)
    (local $h i64)
    (local.set $h (i64.xor (global.get $seed) (global.get $hash)))
    (local.set $h
      (i64.xor (local.get $h)
               (i64.mul (global.get $tick) (i64.const 0x9e3779b97f4a7c15))))
    (local.set $h (i64.xor (local.get $h) (i64.shr_u (local.get $h) (i64.const 30))))
    (local.set $h (i64.mul (local.get $h) (i64.const 0xbf58476d1ce4e5b9)))
    (local.set $h (i64.xor (local.get $h) (i64.shr_u (local.get $h) (i64.const 27))))
    (local.set $h (i64.mul (local.get $h) (i64.const 0x94d049bb133111eb)))
    (i64.xor (local.get $h) (i64.shr_u (local.get $h) (i64.const 31))))

  ;; =========================================================================
  ;; the seven exports
  ;; =========================================================================

  ;; mag_abi_version() -> i32
  ;; The declared ABI version. The host reads this before exchanging any payload
  ;; and refuses the module unless it matches. Three lines: this is the whole cost
  ;; of making a version mismatch a load-time refusal instead of a misread byte.
  (func (export "mag_abi_version") (result i32)
    (i32.const 1))

  ;; mag_alloc(len: i32) -> i32
  (func (export "mag_alloc") (param $len i32) (result i32)
    (call $alloc (local.get $len)))

  ;; mag_free(ptr: i32, len: i32) — no-op for a bump arena, but it must exist
  ;; and it must not trap: the host calls it on every buffer.
  (func (export "mag_free") (param $ptr i32) (param $len i32))

  ;; mag_init(cfg_ptr: i32, cfg_len: i32)
  ;; Derives the module's entropy from the MatchConfig bytes (which carry
  ;; `seed`), then resets the arena. Reads the host's buffer BEFORE the reset.
  (func (export "mag_init") (param $ptr i32) (param $len i32)
    (global.set $seed
      (call $fnv (i64.const 0xcbf29ce484222325) (local.get $ptr) (local.get $len)))
    (global.set $hash (i64.const 0xcbf29ce484222325))
    (global.set $tick (i64.const 0))
    (global.set $bump (i32.const 0x1000)))

  ;; mag_step(payload_ptr: i32, payload_len: i32) -> i32
  ;; Payload: {"tick":N,"inputs":[…]}   Emits: {"rejects":[],"state_hash":N,"tick":N}
  (func (export "mag_step") (param $ptr i32) (param $len i32) (result i32)
    (local $base i32) (local $p i32) (local $t i64)
    ;; 1. consume the host's buffer FIRST — the arena reset below makes it reusable.
    ;;    The tick comes from the payload; it is not counted locally.
    (local.set $t
      (call $int_after (local.get $ptr) (local.get $len) (i32.const 0x150) (i32.const 7)))
    ;; A step must advance. Refusing a mis-sequenced one is the point of being told
    ;; the tick rather than guessing it.
    (if (i64.le_u (local.get $t) (global.get $tick)) (then (unreachable)))
    (global.set $tick (local.get $t))
    (global.set $hash
      (call $fnv (global.get $hash) (local.get $ptr) (local.get $len)))
    ;; 2. only now reset the arena.
    (global.set $bump (i32.const 0x1000))
    ;; 3. build the length-prefixed payload.
    (local.set $base (call $alloc (i32.const 160)))
    (local.set $p (i32.add (local.get $base) (i32.const 4)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x100) (i32.const 27)))
    (local.set $p (call $put_u64 (local.get $p) (call $state_hash)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x144) (i32.const 8)))
    (local.set $p (call $put_u64 (local.get $p) (global.get $tick)))
    (i32.store8 (local.get $p) (i32.const 125)) ;; '}'
    (local.set $p (i32.add (local.get $p) (i32.const 1)))
    (call $finish (local.get $base) (local.get $p)))

  ;; mag_snapshot() -> i32
  ;; Emits {"tick":T,"hash":H} — the complete authoritative state.
  ;; Does not reset the arena, so a buffer previously handed to the host stays
  ;; valid; the contract only requires validity until the next mag_* call.
  (func (export "mag_snapshot") (result i32)
    (local $base i32) (local $p i32)
    (local.set $base (call $alloc (i32.const 96)))
    (local.set $p (i32.add (local.get $base) (i32.const 4)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x120) (i32.const 8)))
    (local.set $p (call $put_u64 (local.get $p) (global.get $tick)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x12c) (i32.const 8)))
    (local.set $p (call $put_u64 (local.get $p) (global.get $hash)))
    (i32.store8 (local.get $p) (i32.const 125)) ;; '}'
    (local.set $p (i32.add (local.get $p) (i32.const 1)))
    (call $finish (local.get $base) (local.get $p)))

  ;; mag_restore(ptr: i32, len: i32)
  ;; Bare JSON, no length prefix — `len` already carries the length. Look up the
  ;; two fields mag_snapshot emitted by name rather than by position, so this does
  ;; not depend on key order.
  (func (export "mag_restore") (param $ptr i32) (param $len i32)
    (global.set $tick
      (call $int_after (local.get $ptr) (local.get $len) (i32.const 0x150) (i32.const 7)))
    (global.set $hash
      (call $int_after (local.get $ptr) (local.get $len) (i32.const 0x158) (i32.const 7))))

  ;; mag_view(player_id: i64) -> i32
  ;; Emits {"player":P,"tick":T} — the only bytes this player may receive.
  (func (export "mag_view") (param $pid i64) (result i32)
    (local $base i32) (local $p i32)
    (local.set $base (call $alloc (i32.const 96)))
    (local.set $p (i32.add (local.get $base) (i32.const 4)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x138) (i32.const 10)))
    (local.set $p (call $put_u64 (local.get $p) (local.get $pid)))
    (local.set $p (call $puts (local.get $p) (i32.const 0x144) (i32.const 8)))
    (local.set $p (call $put_u64 (local.get $p) (global.get $tick)))
    (i32.store8 (local.get $p) (i32.const 125)) ;; '}'
    (local.set $p (i32.add (local.get $p) (i32.const 1)))
    (call $finish (local.get $base) (local.get $p)))
)
