/**
 * magnetite-web-client/src/follow.ts
 *
 * Session follow — the client half of the fleet redirect protocol.
 *
 * When a shard migrates to another node, the node the player is connected to
 * sends `ServerNet::Redirect { redirect }` on the still-open session. The client
 * must reconnect to the new node and present the redirect to be readmitted with
 * the same player id.
 *
 * ## Why this file is paranoid
 *
 * A client that blindly follows a redirect can be walked onto an attacker's
 * node — that is the entire threat this protocol exists to stop. So, before
 * reconnecting anywhere, this module requires ALL of:
 *
 *  1. `issuer` equals the node key this session already pinned. A redirect from
 *     anyone else — including an injected frame — is discarded.
 *  2. The Ed25519 signature over the canonical redirect bytes verifies under
 *     that issuer key.
 *  3. `expires_at` is in the future and `issued_at` is not implausibly ahead of
 *     now.
 *  4. The redirect is addressed to this player.
 *  5. The embedded follow token agrees with the envelope on player, shard,
 *     epoch, target key and issuer — the envelope cannot promise something the
 *     credential does not back.
 *
 * Then, on the NEW connection, `target_key` is pinned: the client asks the far
 * side to sign a fresh nonce (`ClientNet::Hello` → `ServerNet::NodeIdentity`)
 * and aborts unless the key matches and the signature verifies. **The address is
 * a hint; the key is the identity.** Whoever answers at that host:port proves
 * they hold the key or gets nothing.
 *
 * This mirrors `SignedRedirect::verify_for` in `magnetite-runtime/src/cluster.rs`
 * byte for byte — the canonical signing payload below must stay in lockstep with
 * `SignedRedirect::payload` there.
 *
 * ## Crypto
 *
 * Ed25519 verification uses **WebCrypto** (`crypto.subtle`) — no hand-rolled
 * curve arithmetic and no added dependency. Requires a runtime with Ed25519 in
 * WebCrypto (Node 20+, Chrome 137+, Firefox 130+, Safari 17+). Where it is
 * unavailable, `verifyRedirect` throws `RedirectError('unsupported')` and the
 * follow is refused — the client never degrades to following an unverified
 * redirect.
 *
 * ## Not covered
 *
 * NAT traversal: the redirect's address must be directly reachable. And the
 * node-identity proof authenticates the key, not the channel — over plain
 * `ws://` a relay can still sit in the middle. Use `wss://`.
 */

/** Domain separator for redirect signatures — mirrors `REDIRECT_DOMAIN`. */
const REDIRECT_DOMAIN = 'magnetite/fleet/redirect/v1';
/** Domain separator for the node-identity proof — mirrors `NODE_HELLO_DOMAIN`. */
const NODE_HELLO_DOMAIN = 'magnetite-node-hello-v1';
/** Clock skew tolerated on `issued_at`, mirroring `AD_SKEW_SECS`. */
const SKEW_SECS = 60;
/** How long to wait for the new node to prove its key before giving up. */
const DEFAULT_HANDSHAKE_TIMEOUT_MS = 5000;

// ---------------------------------------------------------------------------
// Wire shapes — mirror `SignedRedirect` / its embedded `FollowToken` in Rust
// ---------------------------------------------------------------------------

/** Mirrors the follow credential embedded in a `SignedRedirect`. */
export interface FollowToken {
  player: number;
  shard: number;
  epoch: number;
  target: string;
  issuer: string;
  nonce: string;
  issued_at: number;
  expires_at: number;
  sig: string;
}

/** Mirrors `SignedRedirect` (the JSON body of `ServerNet::Redirect`). */
export interface SignedRedirect {
  player: number;
  shard: number;
  epoch: number;
  addr: string;
  target_key: string;
  issuer: string;
  issued_at: number;
  expires_at: number;
  sig: string;
  token: FollowToken;
}

/** The route a verified redirect resolves to, with the target key pinned. */
export interface FollowRoute {
  addr: string;
  targetKey: string;
  shard: number;
  epoch: number;
}

function toError(e: unknown): Error {
  return e instanceof Error ? e : new Error(String(e));
}

/**
 * A refusal to follow. Every one of these means "stay put / drop it" — there is
 * no partial trust and no fallback path.
 */
export type RedirectErrorCode =
  | 'wrong_issuer'
  | 'bad_signature'
  | 'expired'
  | 'wrong_player'
  | 'token_mismatch'
  | 'bad_address'
  | 'malformed'
  | 'unsupported'
  | 'key_mismatch'
  | 'no_proof';

export class RedirectError extends Error {
  code: RedirectErrorCode;

  constructor(code: RedirectErrorCode, detail?: string) {
    super(detail ? `${code}: ${detail}` : code);
    this.name = 'RedirectError';
    this.code = code;
  }
}

// ---------------------------------------------------------------------------
// Encoding helpers — must match the Rust canonical payload exactly
// ---------------------------------------------------------------------------

export function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
  if (typeof hex !== 'string' || hex.length % 2 !== 0 || /[^0-9a-fA-F]/.test(hex)) {
    throw new RedirectError('malformed', 'not a hex string');
  }
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return out;
}

/** Little-endian u64, via BigInt so ids above 2^53 survive. */
function u64le(n: number): Uint8Array<ArrayBuffer> {
  const b = new Uint8Array(8);
  let v = BigInt(n);
  if (v < 0n) throw new RedirectError('malformed', 'negative u64');
  for (let i = 0; i < 8; i++) {
    b[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return b;
}

/** Little-endian u32. */
function u32le(n: number): Uint8Array<ArrayBuffer> {
  const b = new Uint8Array(4);
  let v = Number(n) >>> 0;
  for (let i = 0; i < 4; i++) {
    b[i] = v & 0xff;
    v >>>= 8;
  }
  return b;
}

/** Length-prefixed bytes — mirrors `push_bytes` (u32 LE length, then body). */
function lenPrefixed(bytes: Uint8Array<ArrayBuffer>): Uint8Array<ArrayBuffer> {
  return concat(u32le(bytes.length), bytes);
}

function concat(...parts: Uint8Array<ArrayBuffer>[]): Uint8Array<ArrayBuffer> {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

const utf8 = (s: string): Uint8Array<ArrayBuffer> => new TextEncoder().encode(s);

/**
 * The canonical bytes covered by a redirect's signature.
 *
 * Mirrors `SignedRedirect::payload`:
 * `DOMAIN || player u64le || shard u32le || epoch u64le || len(addr) || addr
 *  || target_key[32] || issuer[32] || issued_at u64le || expires_at u64le
 *  || token.sig[64]`
 *
 * The token's own signature is folded in, so envelope and credential cannot be
 * mixed and matched.
 *
 * @param r - a `SignedRedirect` JSON body
 */
export function redirectSigningBytes(r: object): Uint8Array<ArrayBuffer> {
  const sr = r as SignedRedirect;
  if (!sr || typeof sr !== 'object' || !sr.token) {
    throw new RedirectError('malformed', 'redirect has no token');
  }
  return concat(
    utf8(REDIRECT_DOMAIN),
    u64le(sr.player),
    u32le(sr.shard),
    u64le(sr.epoch),
    lenPrefixed(utf8(String(sr.addr ?? ''))),
    hexToBytes(sr.target_key),
    hexToBytes(sr.issuer),
    u64le(sr.issued_at),
    u64le(sr.expires_at),
    hexToBytes(sr.token.sig),
  );
}

/** Bytes a node signs to answer `ClientNet::Hello`. */
export function nodeHelloBytes(nonce: string, nodeKeyHex: string): Uint8Array<ArrayBuffer> {
  return concat(utf8(NODE_HELLO_DOMAIN), utf8(nonce), hexToBytes(nodeKeyHex));
}

// ---------------------------------------------------------------------------
// Ed25519 verification (WebCrypto — no hand-rolled curve arithmetic)
// ---------------------------------------------------------------------------

function subtle(): SubtleCrypto {
  const c = globalThis.crypto;
  if (!c || !c.subtle) {
    throw new RedirectError('unsupported', 'WebCrypto unavailable — refusing to follow unverified');
  }
  return c.subtle;
}

/**
 * Verify an Ed25519 signature. Returns false on a bad signature; throws
 * `RedirectError('unsupported')` if the runtime cannot do Ed25519 at all —
 * "cannot check" is never treated as "checks out".
 *
 * @param pubKeyHex - 32-byte hex
 * @param sigHex - 64-byte hex
 */
export async function verifyEd25519(pubKeyHex: string, message: Uint8Array<ArrayBuffer>, sigHex: string): Promise<boolean> {
  const raw = hexToBytes(pubKeyHex);
  const sig = hexToBytes(sigHex);
  if (raw.length !== 32) throw new RedirectError('malformed', 'public key must be 32 bytes');
  if (sig.length !== 64) throw new RedirectError('malformed', 'signature must be 64 bytes');

  let key: CryptoKey;
  try {
    key = await subtle().importKey('raw', raw, { name: 'Ed25519' }, false, ['verify']);
  } catch (e) {
    if (e instanceof RedirectError) throw e;
    throw new RedirectError('unsupported', `Ed25519 not supported by this runtime: ${toError(e).message}`);
  }
  return subtle().verify({ name: 'Ed25519' }, key, sig, message);
}

// ---------------------------------------------------------------------------
// Redirect verification
// ---------------------------------------------------------------------------

const nowSecs = (): number => Math.floor(Date.now() / 1000);

export interface VerifyRedirectOpts {
  /** hex node key this session is authenticated to */
  issuerKey: string;
  /** our player id */
  playerId: number;
  /** unix seconds (injectable for tests) */
  now?: number;
}

/**
 * Verify a redirect and return the route to reconnect to, with the target key
 * pinned. Throws `RedirectError` on any failure — there is no "probably fine".
 *
 * @param redirect - the `SignedRedirect` JSON body
 */
export async function verifyRedirect(redirect: object, { issuerKey, playerId, now = nowSecs() }: VerifyRedirectOpts): Promise<FollowRoute> {
  if (!redirect || typeof redirect !== 'object') {
    throw new RedirectError('malformed', 'not an object');
  }
  const r = redirect as SignedRedirect;
  // 1. Did the node we are actually talking to say this? Checking the issuer
  //    first is what makes an injected redirect inert: an attacker who can push
  //    a frame into the session still cannot sign as the node.
  if (String(r.issuer).toLowerCase() !== String(issuerKey).toLowerCase()) {
    throw new RedirectError('wrong_issuer', `signed by ${r.issuer}, expected ${issuerKey}`);
  }
  // 2. Authorship.
  const ok = await verifyEd25519(r.issuer, redirectSigningBytes(r), r.sig);
  if (!ok) throw new RedirectError('bad_signature');
  // 3. Freshness. A lapsed redirect is refused even though it verifies — a
  //    captured one must not be usable later.
  if (Number(r.expires_at) <= now) throw new RedirectError('expired', 'expires_at passed');
  if (Number(r.issued_at) > now + SKEW_SECS) {
    throw new RedirectError('expired', 'issued_at is in the future');
  }
  // 4. Addressed to us.
  if (String(r.player) !== String(playerId)) throw new RedirectError('wrong_player');
  // 5. The envelope must not promise what the credential does not back.
  const t = r.token;
  const checks: Array<[string, unknown, unknown]> = [
    ['player', t.player, r.player],
    ['shard', t.shard, r.shard],
    ['epoch', t.epoch, r.epoch],
    ['target', t.target, r.target_key],
    ['issuer', t.issuer, r.issuer],
  ];
  for (const [field, a, b] of checks) {
    if (String(a) !== String(b)) throw new RedirectError('token_mismatch', field);
  }
  if (!r.addr || !String(r.addr).trim()) throw new RedirectError('bad_address');

  return {
    addr: String(r.addr),
    targetKey: String(r.target_key),
    shard: Number(r.shard),
    epoch: Number(r.epoch),
  };
}

// ---------------------------------------------------------------------------
// Following: reconnect, pin the target key, present the credential
// ---------------------------------------------------------------------------

function randomNonce(): string {
  const b = new Uint8Array(16);
  const c: Crypto | undefined = globalThis.crypto;
  c?.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
}

/**
 * Turn a redirect's `host:port` into a WebSocket URL, keeping the scheme of the
 * connection we are following from (so a `wss://` session never silently
 * downgrades to `ws://`).
 *
 * @param addr - `host:port`
 * @param currentUrl - the URL of the session being redirected away from
 */
export function redirectUrl(addr: string, currentUrl?: string): string {
  const secure = /^wss:/i.test(String(currentUrl || ''));
  return `${secure ? 'wss' : 'ws'}://${addr}`;
}

export interface FollowRedirectOpts {
  /** ws URL of the target node */
  url: string;
  /** hex node key pinned from the redirect */
  targetKey: string;
  /** the verified `SignedRedirect` body */
  redirect: object;
  /** injectable for tests */
  openSocket?: ((url: string) => WebSocket) | undefined;
  timeoutMs?: number | undefined;
}

/**
 * Follow a verified redirect: open a socket to the target, make it prove it
 * holds `targetKey`, then present the redirect.
 *
 * Aborts (and closes the socket) if the far side presents a different key or
 * cannot sign our nonce — the address got us there, the key decides whether we
 * stay.
 *
 * @returns an open socket, already admitted
 */
export function followRedirect({
  url,
  targetKey,
  redirect,
  openSocket = (u: string) => new WebSocket(u),
  timeoutMs = DEFAULT_HANDSHAKE_TIMEOUT_MS,
}: FollowRedirectOpts): Promise<WebSocket> {
  return new Promise<WebSocket>((resolve, reject) => {
    const ws = openSocket(url);
    const nonce = randomNonce();
    let settled = false;

    const fail = (err: Error): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        ws.close(1000, 'follow aborted');
      } catch {
        /* already closing */
      }
      reject(err);
    };

    const timer = setTimeout(
      () => fail(new RedirectError('no_proof', 'target did not prove its node key in time')),
      timeoutMs,
    );

    ws.addEventListener('open', () => {
      ws.send(JSON.stringify({ type: 'hello', nonce }));
    });

    // addEventListener wants a `(event) => void` listener; the handler body
    // needs `await` for the signature check below, so it's declared async
    // and invoked through a sync wrapper (`void handleMessage(...)`) rather
    // than passed directly — every path inside is already caught, so there
    // is nothing for the wrapper itself to await or handle.
    async function handleMessage(event: MessageEvent<string>) {
      if (settled) return;
      let msg;
      try {
        msg = JSON.parse(event.data);
      } catch {
        return;
      }

      if (msg.type === 'node_identity') {
        // THE pin. An impostor at this address answers here, and gets refused.
        if (String(msg.node_key).toLowerCase() !== String(targetKey).toLowerCase()) {
          fail(
            new RedirectError('key_mismatch', `target presented ${msg.node_key}, expected ${targetKey}`),
          );
          return;
        }
        if (msg.nonce !== nonce) {
          fail(new RedirectError('no_proof', 'node echoed the wrong nonce'));
          return;
        }
        let good;
        try {
          good = await verifyEd25519(msg.node_key, nodeHelloBytes(nonce, msg.node_key), msg.sig);
        } catch (e) {
          fail(toError(e));
          return;
        }
        if (!good) {
          fail(new RedirectError('no_proof', 'node-identity signature does not verify'));
          return;
        }
        // Proven. Now — and only now — hand over the credential.
        ws.send(JSON.stringify({ type: 'follow', redirect }));
        return;
      }

      if (msg.type === 'welcome') {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(ws);
      }
    }
    ws.addEventListener('message', (event: MessageEvent<string>) => { void handleMessage(event); });

    ws.addEventListener('close', () =>
      fail(new RedirectError('no_proof', 'target closed the connection — follow refused')),
    );
    ws.addEventListener('error', () => fail(new RedirectError('no_proof', 'socket error')));
  });
}

export interface VerifyAndFollowOpts {
  issuerKey: string;
  playerId: number;
  now?: number;
  /** URL of the session being redirected away from */
  currentUrl?: string;
  openSocket?: (url: string) => WebSocket;
  timeoutMs?: number;
}

/**
 * The whole client-side flow, in one call: verify, then follow.
 *
 * @param opts - see {@link verifyRedirect}, plus `currentUrl` and the
 *   `openSocket` / `timeoutMs` overrides of {@link followRedirect}.
 */
export async function verifyAndFollow(
  redirect: object,
  opts: VerifyAndFollowOpts,
): Promise<{ socket: WebSocket; route: FollowRoute }> {
  const route = await verifyRedirect(redirect, opts);
  const socket = await followRedirect({
    url: redirectUrl(route.addr, opts.currentUrl),
    targetKey: route.targetKey,
    redirect,
    openSocket: opts.openSocket,
    timeoutMs: opts.timeoutMs,
  });
  return { socket, route };
}
