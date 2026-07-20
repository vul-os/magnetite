/**
 * Client-side session follow.
 *
 * These are cross-language tests: the redirect under test was signed by the
 * Rust runtime (`cargo run --example redirect_fixture`), so a passing suite
 * proves the JS canonical encoding matches `SignedRedirect::payload` byte for
 * byte — not merely that this file agrees with itself.
 */

import { describe, it, expect, vi } from 'vitest';

import fixture from './__fixtures__/redirect.json';
import {
  RedirectError,
  followRedirect,
  hexToBytes,
  nodeHelloBytes,
  redirectSigningBytes,
  redirectUrl,
  verifyAndFollow,
  verifyEd25519,
  verifyRedirect,
} from './follow.js';

const { redirect, issuer_key: ISSUER, target_key: TARGET, player: PLAYER } = fixture;
/** A moment inside the fixture's validity window. */
const FRESH = fixture.issued_at + 1;

const opts = (over = {}) => ({ issuerKey: ISSUER, playerId: PLAYER, now: FRESH, ...over });

// ---------------------------------------------------------------------------
// Cross-language signature verification
// ---------------------------------------------------------------------------

describe('canonical encoding', () => {
  it('verifies a signature produced by the Rust runtime', async () => {
    // The whole point: our bytes are Rust's bytes.
    await expect(
      verifyEd25519(redirect.issuer, redirectSigningBytes(redirect), redirect.sig),
    ).resolves.toBe(true);
  });

  it('rejects the signature if a single field is altered', async () => {
    const tampered = { ...redirect, addr: 'attacker.example:7100' };
    await expect(
      verifyEd25519(tampered.issuer, redirectSigningBytes(tampered), tampered.sig),
    ).resolves.toBe(false);
  });

  it('verifies the node-identity proof produced by the Rust runtime', async () => {
    const { nonce, node_key: key, sig } = fixture.hello;
    await expect(verifyEd25519(key, nodeHelloBytes(nonce, key), sig)).resolves.toBe(true);
    // A different nonce is a different statement — the proof does not replay.
    await expect(verifyEd25519(key, nodeHelloBytes('other', key), sig)).resolves.toBe(false);
  });

  it('refuses malformed hex rather than guessing', () => {
    expect(() => hexToBytes('zz')).toThrow(RedirectError);
    expect(() => hexToBytes('abc')).toThrow(RedirectError);
  });
});

// ---------------------------------------------------------------------------
// verifyRedirect — the gate
// ---------------------------------------------------------------------------

describe('verifyRedirect', () => {
  it('accepts a genuine redirect and pins the target key', async () => {
    const route = await verifyRedirect(redirect, opts());
    expect(route.addr).toBe('10.0.0.11:7100');
    expect(route.targetKey).toBe(TARGET);
    expect(route.shard).toBe(redirect.shard);
    expect(route.epoch).toBe(redirect.epoch);
  });

  it('refuses a redirect signed by anyone but the node we are talking to', async () => {
    // The forged-redirect case: an attacker who can inject a frame into the
    // session still cannot sign as the node.
    const otherNode = 'aa'.repeat(32);
    await expect(verifyRedirect(redirect, opts({ issuerKey: otherNode }))).rejects.toMatchObject({
      code: 'wrong_issuer',
    });
  });

  it('refuses a redirect whose signature does not verify', async () => {
    const tampered = { ...redirect, addr: 'attacker.example:7100' };
    await expect(verifyRedirect(tampered, opts())).rejects.toMatchObject({
      code: 'bad_signature',
    });
  });

  it('refuses a redirect that re-points the target key', async () => {
    // Swapping in an attacker's node key breaks the envelope signature — this
    // is why the client verifies rather than trusting `target_key` on sight.
    const hijack = { ...redirect, target_key: 'bb'.repeat(32) };
    await expect(verifyRedirect(hijack, opts())).rejects.toMatchObject({
      code: 'bad_signature',
    });
  });

  it('refuses an expired redirect even though it verifies', async () => {
    await expect(
      verifyRedirect(redirect, opts({ now: fixture.expires_at + 1 })),
    ).rejects.toMatchObject({ code: 'expired' });
  });

  it('refuses a future-dated redirect beyond tolerated skew', async () => {
    await expect(
      verifyRedirect(redirect, opts({ now: fixture.issued_at - 3600 })),
    ).rejects.toMatchObject({ code: 'expired' });
  });

  it('refuses a redirect addressed to another player', async () => {
    await expect(verifyRedirect(redirect, opts({ playerId: PLAYER + 1 }))).rejects.toMatchObject({
      code: 'wrong_player',
    });
  });

  it('refuses when the token disagrees with the envelope', async () => {
    const mismatched = { ...redirect, token: { ...redirect.token, shard: redirect.shard + 1 } };
    // The token sig is folded into the envelope payload, so this trips the
    // signature check first — either way it is refused, never followed.
    await expect(verifyRedirect(mismatched, opts())).rejects.toBeInstanceOf(RedirectError);
  });

  it('refuses garbage instead of throwing something unhelpful', async () => {
    await expect(verifyRedirect(null, opts())).rejects.toMatchObject({ code: 'malformed' });
    await expect(verifyRedirect({ issuer: ISSUER }, opts())).rejects.toBeInstanceOf(RedirectError);
  });
});

// ---------------------------------------------------------------------------
// followRedirect — key pinning on the new connection
// ---------------------------------------------------------------------------

/** Minimal scriptable WebSocket double. */
class FakeSocket {
  constructor() {
    this.sent = [];
    this.closed = false;
    this._listeners = { open: [], message: [], close: [], error: [] };
  }
  addEventListener(type, fn) {
    this._listeners[type].push(fn);
  }
  send(data) {
    this.sent.push(JSON.parse(data));
  }
  close() {
    this.closed = true;
  }
  emit(type, event) {
    for (const fn of this._listeners[type]) fn(event);
  }
  /** Deliver a server frame and let its async handler settle. */
  async deliver(obj) {
    this.emit('message', { data: JSON.stringify(obj) });
    await new Promise((r) => setTimeout(r, 0));
  }
  get lastSent() {
    return this.sent[this.sent.length - 1];
  }
}

/** Drive the far side as an honest target node holding `key`. */
async function honestTarget(sock, { key = TARGET, sig = fixture.hello.sig } = {}) {
  sock.emit('open');
  const hello = sock.lastSent;
  await sock.deliver({ type: 'node_identity', node_key: key, nonce: hello.nonce, sig });
  return hello;
}

describe('followRedirect', () => {
  it('proves the key before handing over the credential', async () => {
    const sock = new FakeSocket();
    // The fixture's hello proof is over a fixed nonce, so sign-check must be
    // driven with that nonce for the honest path.
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
    });
    promise.catch(() => {}); // attach a handler before driving, so failures are not "unhandled"
    sock.emit('open');
    expect(sock.lastSent.type).toBe('hello');
    // Answer with a real proof over the nonce the client actually chose — we
    // cannot forge that here, so assert the ordering property instead: nothing
    // is sent until node_identity is validated.
    expect(sock.sent.some((m) => m.type === 'follow')).toBe(false);

    // A wrong-key answer must abort.
    await sock.deliver({
      type: 'node_identity',
      node_key: 'cc'.repeat(32),
      nonce: sock.sent[0].nonce,
      sig: fixture.hello.sig,
    });
    await expect(promise).rejects.toMatchObject({ code: 'key_mismatch' });
    expect(sock.sent.some((m) => m.type === 'follow')).toBe(false);
    expect(sock.closed).toBe(true);
  });

  it('aborts when the far side cannot sign our nonce', async () => {
    // Right key, wrong signature: an attacker who knows the target's public key
    // (it is public!) but not its secret gets nothing.
    const sock = new FakeSocket();
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
    });
    promise.catch(() => {});
    await honestTarget(sock); // fixture sig is over a different nonce → invalid
    await expect(promise).rejects.toMatchObject({ code: 'no_proof' });
    expect(sock.sent.some((m) => m.type === 'follow')).toBe(false);
  });

  it('aborts when the nonce is not echoed back', async () => {
    const sock = new FakeSocket();
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
    });
    promise.catch(() => {}); // attach a handler before driving, so failures are not "unhandled"
    sock.emit('open');
    await sock.deliver({
      type: 'node_identity',
      node_key: TARGET,
      nonce: 'not-the-one-we-sent',
      sig: fixture.hello.sig,
    });
    await expect(promise).rejects.toMatchObject({ code: 'no_proof' });
  });

  it('treats a closed connection as a refusal', async () => {
    const sock = new FakeSocket();
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
    });
    promise.catch(() => {}); // attach a handler before driving, so failures are not "unhandled"
    sock.emit('open');
    sock.emit('close', {});
    await expect(promise).rejects.toMatchObject({ code: 'no_proof' });
  });

  it('gives up rather than hanging if the target never proves itself', async () => {
    vi.useFakeTimers();
    const sock = new FakeSocket();
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
      timeoutMs: 100,
    });
    promise.catch(() => {}); // attach a handler before driving, so failures are not "unhandled"
    sock.emit('open');
    vi.advanceTimersByTime(200);
    await expect(promise).rejects.toMatchObject({ code: 'no_proof' });
    vi.useRealTimers();
  });

  it('resolves once the target welcomes us back', async () => {
    // Drive the honest path with a proof over the client's own nonce by
    // stubbing only the crypto boundary — everything else is the real flow.
    const sock = new FakeSocket();
    const promise = followRedirect({
      url: 'ws://10.0.0.11:7100',
      targetKey: TARGET,
      redirect,
      openSocket: () => sock,
    });
    promise.catch(() => {}); // attach a handler before driving, so failures are not "unhandled"
    sock.emit('open');
    const nonce = sock.sent[0].nonce;
    expect(nonce).toMatch(/^[0-9a-f]{32}$/);

    const spy = vi.spyOn(globalThis.crypto.subtle, 'verify').mockResolvedValue(true);
    await sock.deliver({ type: 'node_identity', node_key: TARGET, nonce, sig: fixture.hello.sig });
    expect(sock.lastSent.type).toBe('follow');
    expect(sock.lastSent.redirect).toEqual(redirect);
    await sock.deliver({ type: 'welcome', player_id: PLAYER, config: {} });
    await expect(promise).resolves.toBe(sock);
    spy.mockRestore();
  });
});

// ---------------------------------------------------------------------------
// verifyAndFollow + URL handling
// ---------------------------------------------------------------------------

describe('verifyAndFollow', () => {
  it('never opens a socket for a redirect that fails verification', async () => {
    const openSocket = vi.fn(() => new FakeSocket());
    await expect(
      verifyAndFollow(redirect, opts({ now: fixture.expires_at + 1, openSocket })),
    ).rejects.toMatchObject({ code: 'expired' });
    expect(openSocket).not.toHaveBeenCalled();
  });
});

describe('redirectUrl', () => {
  it('keeps a secure session secure', () => {
    expect(redirectUrl('10.0.0.11:7100', 'wss://a.example:9000')).toBe('wss://10.0.0.11:7100');
    expect(redirectUrl('10.0.0.11:7100', 'ws://a.example:9000')).toBe('ws://10.0.0.11:7100');
  });
});
