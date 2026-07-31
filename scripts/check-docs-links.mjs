#!/usr/bin/env node
// Guard: every markdown chapter under docs/ must be reachable, and every
// relative link/image inside a markdown file must resolve on disk.
//
// Why this exists
// ----------------
// An earlier audit found 16 real doc chapters (7 of 8 docs/moat/*.md, plus 14
// standalone feature/assessment pages) that existed, were real content, and
// had ZERO inbound links from anywhere in the docs tree — a reader could only
// reach them by guessing the URL or grepping the repo. They were hand-linked
// into docs/index.md, but nothing stopped the next new chapter from landing
// the same way. This script is that backstop.
//
// It checks two independent things, both of which fail the build:
//
//   1. INBOUND LINKS — every `docs/**/*.md` file must be the target of at
//      least one relative markdown link (`[text](path)`) or image
//      (`![alt](path)`) from some OTHER markdown file anywhere in the repo
//      (docs/, site/docs/, or root-level .md files like README.md all count
//      as valid link sources — README.md links straight into docs/ for
//      several chapters, and that's a legitimate inbound link).
//
//   2. LINK INTEGRITY — every relative link/image target found in ANY scanned
//      markdown file must resolve to a real file on disk. A link to
//      `./nonexistent.md` fails the build exactly like an unlinked chapter
//      does; a broken link is just an inbound-link promise nobody kept.
//
// External links (http://, https://, mailto:, etc.), in-page anchors (#foo)
// and the bare directory index convention are not checked for existence
// (nothing to check on disk) — only relative filesystem paths are.
//
// Fails CLOSED: zero markdown files found, zero links found, or a missing
// docs/ directory are all errors, never a silent pass. Prints an explicit
// coverage count on every run so "examined nothing, printed PASS" cannot
// happen unnoticed (see FANOUT-LOOP-STATE.md's "guards that check nothing"
// lesson — this script is written specifically against that failure mode).

import { readFileSync, existsSync } from 'node:fs';
import { readdir } from 'node:fs/promises';
import { dirname, join, relative, resolve, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DOCS_DIR = join(ROOT, 'docs');

// Directories never walked when collecting link SOURCES (build output,
// dependencies, vendored/generated trees).
const SKIP_DIR_NAMES = new Set(['node_modules', 'target', 'dist', '.git', 'test-results']);

// Files under docs/ that are legitimately unlinked and must stay that way
// without failing the build. Keep this list SMALL and each entry justified —
// it exists so a genuinely new orphaned chapter fails loudly instead of the
// allowlist quietly growing to swallow it.
//
//   - docs/project/*.md (except index.md, which IS linked from docs/index.md):
//     self-labelled STALE/HISTORICAL program records. They are in fact linked
//     today (from docs/project/index.md), so this entry is a deliberate
//     safety margin, not a cover for a real gap: if a future reorg of
//     docs/project/index.md's table drops one, that is an editorial choice
//     about a historical-archive file, not the "reader can never find this"
//     failure this script exists to catch.
//
//   - docs/api-reference/auth.md, docs/comms/streaming.md: found while
//     writing this script (2026-07-31) — real, pre-redesign chapters whose
//     *content* is wrong (fabricated custodial wallet balances; a REST API
//     table naming routes that do not exist in backend/src/main.rs), now
//     superseded by docs/api-reference/index.md and docs/comms/index.md
//     respectively. Each carries an in-file "STALE — superseded" notice
//     pointing at the current chapter. Deliberately not linked — linking a
//     chapter that contradicts the platform's own non-custodial claim would
//     be a worse failure than leaving it unreachable.
//
//   - docs/screenshots/README.md: an earlier draft of docs/screenshots.md
//     (the maintained, linked chapter), left in place as a directory-local
//     note. Marked superseded in-file; not linked on purpose.
const ALLOWLIST = new Set([
  'docs/project/AUDIT.md',
  'docs/project/DECENTRALIZATION_PROGRESS.md',
  'docs/project/DECISIONS.md',
  'docs/project/GAPS.md',
  'docs/project/roadmap.md',
  'docs/project/TASKS.md',
  'docs/api-reference/auth.md',
  'docs/comms/streaming.md',
  'docs/screenshots/README.md',
]);

const args = new Set(process.argv.slice(2));
const quiet = args.has('--quiet');
const log = (...a) => { if (!quiet) console.log(...a); };

function fail(msg) {
  console.error(`FAIL: ${msg}`);
  process.exit(1);
}

/** Recursively collect every *.md file under `dir`, skipping noise dirs. */
async function collectMarkdown(dir) {
  const out = [];
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    if (SKIP_DIR_NAMES.has(e.name)) continue;
    const full = join(dir, e.name);
    if (e.isDirectory()) {
      out.push(...await collectMarkdown(full));
    } else if (e.isFile() && e.name.toLowerCase().endsWith('.md')) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Extract relative link/image targets from markdown source.
 * Matches `[text](target)` and `![alt](target)`. Skips absolute URLs
 * (scheme://...), mailto:, and bare in-page anchors (#foo).
 * Strips a trailing `#anchor` or a leading `<...>` angle-bracket wrapper.
 */
function extractRelativeTargets(source) {
  const targets = [];
  // Matches ]( ...content... ) — content up to the first unescaped ')' or a
  // quoted title. Good enough for hand-written docs markdown (no nested
  // parens in link targets anywhere in this tree, checked below by the
  // "unresolved" tally: a malformed match would show up as a bogus path).
  const re = /!?\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g;
  let m;
  while ((m = re.exec(source)) !== null) {
    let target = m[1].trim();
    if (!target || target.startsWith('#')) continue;
    if (/^[a-z][a-z0-9+.-]*:/i.test(target)) continue; // scheme:// or mailto:
    // Strip a trailing in-page anchor: ./foo.md#section -> ./foo.md
    const hashIdx = target.indexOf('#');
    if (hashIdx !== -1) target = target.slice(0, hashIdx);
    if (!target) continue; // was purely an anchor into the SAME file
    targets.push(target);
  }
  return targets;
}

async function main() {
  if (!existsSync(DOCS_DIR)) fail(`docs/ directory does not exist at ${DOCS_DIR}`);

  // Link SOURCES: every markdown file in the repo, not just docs/ — README.md
  // and DECENTRALIZATION.md at the root both link straight into docs/*.md,
  // and site/docs/*.md mirrors are real inbound sources too.
  const allMarkdown = await collectMarkdown(ROOT);
  if (allMarkdown.length === 0) fail('found 0 markdown files in the repo — the scan itself is broken.');

  // Link TARGETS that must be reachable: docs/**/*.md only.
  const docsMarkdown = allMarkdown.filter((f) => f.startsWith(DOCS_DIR + '/') || f === DOCS_DIR);
  if (docsMarkdown.length === 0) fail('found 0 markdown files under docs/ — the scan itself is broken.');

  const docsMarkdownSet = new Set(docsMarkdown);
  const inbound = new Map(docsMarkdown.map((f) => [f, 0]));

  let totalLinks = 0;
  let resolvedLinks = 0;
  const broken = []; // { from, target, resolvedPath }

  const SITE_DOCS_DIR = join(ROOT, 'site', 'docs');
  const SITE_DIR = join(ROOT, 'site');

  for (const file of allMarkdown) {
    const source = readFileSync(file, 'utf8');
    const targets = extractRelativeTargets(source);

    // site/docs/*.md is fetched at runtime by site/docs.html and injected
    // into that shell page (see site/docs.html's `fetch('./docs/'+slug+
    // '.md')`) — the browser's location never becomes the markdown file's own
    // path, so a relative link written inside one of these files resolves
    // against site/ (the shell's directory), not against site/docs/ (the
    // markdown source's own directory). Every other markdown file in the
    // repo is read from disk (by a human, an editor, or this very script) at
    // its own real path, so the ordinary rule applies to it.
    const baseDir = dirname(file) === SITE_DOCS_DIR ? SITE_DIR : dirname(file);

    for (const target of targets) {
      totalLinks++;
      const resolved = resolve(baseDir, target);

      // Does it resolve on disk at all? A directory target is fine as-is
      // (e.g. READMEs routinely link a bare `game-templates/authoritative/`
      // expecting GitHub's own directory listing) — no index file required.
      const onDisk = existsSync(resolved);
      if (!onDisk) {
        broken.push({ from: relative(ROOT, file), target, resolved: relative(ROOT, resolved) });
        continue;
      }
      resolvedLinks++;

      // Record inbound credit if the resolved target is a docs/**/*.md file
      // we're tracking (or the directory-index form of one).
      let creditPath = resolved;
      if (extname(creditPath).toLowerCase() !== '.md') {
        const idx = join(creditPath, 'index.md');
        if (docsMarkdownSet.has(idx)) creditPath = idx;
      }
      if (docsMarkdownSet.has(creditPath) && creditPath !== file) {
        inbound.set(creditPath, (inbound.get(creditPath) ?? 0) + 1);
      }
    }
  }

  const unlinked = docsMarkdown
    .filter((f) => (inbound.get(f) ?? 0) === 0)
    .map((f) => relative(ROOT, f))
    .filter((rel) => !ALLOWLIST.has(rel))
    .sort();

  const allowlistedButActuallyLinked = [...ALLOWLIST].filter((rel) => {
    const abs = join(ROOT, rel);
    return docsMarkdownSet.has(abs) && (inbound.get(abs) ?? 0) > 0;
  });

  // --- Report -----------------------------------------------------------
  log('Docs link coverage');
  log(`  markdown files scanned (repo-wide, as link sources): ${allMarkdown.length}`);
  log(`  markdown files under docs/ (link targets tracked)  : ${docsMarkdown.length}`);
  log(`  relative links/images examined                     : ${totalLinks}`);
  log(`  of which resolved on disk                           : ${resolvedLinks}`);
  log(`  allowlisted-as-unlinked entries                     : ${ALLOWLIST.size}`);

  if (allowlistedButActuallyLinked.length) {
    log(
      `  note: ${allowlistedButActuallyLinked.length} allowlist entr${allowlistedButActuallyLinked.length === 1 ? 'y is' : 'ies are'} ` +
      `now genuinely linked (kept in the allowlist as a documented safety margin — see the comment above ALLOWLIST):`
    );
    for (const rel of allowlistedButActuallyLinked) log(`    - ${rel}`);
  }

  // Coverage must be non-zero or this guard is examining nothing.
  if (docsMarkdown.length === 0) fail('0 docs/ markdown files examined — coverage count is zero, refusing to pass.');
  if (totalLinks === 0) fail('0 links examined across the whole repo — coverage count is zero, refusing to pass.');

  let status = 0;

  if (broken.length) {
    console.error(`\nFAIL: ${broken.length} relative link(s)/image(s) do not resolve on disk:`);
    for (const b of broken) {
      console.error(`  - ${b.from} -> "${b.target}" (resolved: ${b.resolved})`);
    }
    status = 1;
  }

  if (unlinked.length) {
    console.error(
      `\nFAIL: ${unlinked.length} docs/ chapter(s) have NO inbound link from any other markdown file ` +
      `in the repo:`
    );
    for (const rel of unlinked) console.error(`  - ${rel}`);
    console.error(
      '\nLink each of these from an appropriate index/hub page (docs/index.md or the ' +
      "relevant subdirectory's index.md), or — only if it is genuinely meant to be " +
      'unreachable by design — add it to ALLOWLIST in scripts/check-docs-links.mjs with ' +
      'a one-line justification next to the existing entries.'
    );
    status = 1;
  }

  if (status === 0) {
    log(
      `\nOK — ${docsMarkdown.length} docs/ chapters all reachable, ${resolvedLinks}/${totalLinks} relative links resolve.`
    );
  }

  process.exit(status);
}

main().catch((err) => {
  console.error('check-docs-links:', err.stack || err.message);
  process.exit(1);
});
