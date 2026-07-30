//! `serve-web-bundle` — point it at a directory of web-export files and it
//! serves them as a content-addressed rung-0 package.
//!
//! ```text
//! serve-web-bundle <dir> [--addr 127.0.0.1:8080] [--entry index.html]
//!                        [--paid] [--paid-item <item>] [--no-isolation]
//! ```
//!
//! Every file under `<dir>` is read, hashed into a `LocalBlobStore`, and listed
//! in a manifest; the root hash of that manifest becomes the URL. Nothing is
//! written to disk, no database is opened, no network call is made, and no
//! account exists. That is the point: ALIGNMENT.md §1 requires a developer with a
//! laptop to be able to host alone.
//!
//! `--paid` prices the bundle at its own root hash
//! (`Pricing::paid_for_bundle`) and prints a ready-to-use `MockPaymentRail`
//! receipt cookie, so the gate can be exercised end to end in a browser with no
//! chain and no wallet. That receipt is worth nothing outside this process — the
//! mock rail is a deterministic offline stub, per `site/docs/status.md`.
//!
//! In-memory storage means the whole bundle is resident. Fine for a dev loop and
//! for anything itch.io-shaped; swap `LocalBlobStore` for
//! `FsBlobStore`/a Walrus binding for a real catalogue.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use magnetite_seams::blobstore::LocalBlobStore;
use magnetite_seams::identity::{PubKey, RawKeypairAuth};
use magnetite_seams::payment::{MockPaymentRail, PaymentRail, PaymentSplit, Split};
use magnetite_web_host::{
    host::WebHost,
    ingest,
    manifest::{BundleKind, BundleManifest, Pricing},
    respond::{CrossOriginIsolation, HostedBundle, ServePolicy},
    RECEIPT_COOKIE,
};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("serve-web-bundle: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut addr: SocketAddr = "127.0.0.1:8080".parse().expect("literal addr parses");
    let mut entry = "index.html".to_string();
    let mut paid_item: Option<String> = None;
    let mut paid_from_root = false;
    let mut isolation = CrossOriginIsolation::Enabled;

    while let Some(a) = args.next() {
        match a.as_str() {
            "--addr" => {
                addr = args
                    .next()
                    .ok_or("--addr needs a value")?
                    .parse()
                    .map_err(|e| format!("--addr: {e}"))?
            }
            "--entry" => entry = args.next().ok_or("--entry needs a value")?,
            "--paid" => paid_from_root = true,
            "--paid-item" => paid_item = Some(args.next().ok_or("--paid-item needs a value")?),
            "--no-isolation" => isolation = CrossOriginIsolation::Disabled,
            "-h" | "--help" => {
                println!(
                    "serve-web-bundle <dir> [--addr HOST:PORT] [--entry PATH] \
                     [--paid] [--paid-item ITEM] [--no-isolation]"
                );
                return Ok(());
            }
            other if other.starts_with('-') => return Err(format!("unknown flag {other}")),
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let dir = dir.ok_or("usage: serve-web-bundle <dir> [--addr HOST:PORT]")?;

    // 1. Read and hash every file.
    let blobs = LocalBlobStore::new();
    let mut files = Vec::new();
    let mut paths = Vec::new();
    collect(&dir, &dir, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{} contains no files", dir.display()));
    }
    for rel in &paths {
        let bytes = std::fs::read(dir.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        files.push(ingest(&blobs, rel.clone(), &bytes).await);
    }

    // 2. Manifest, then pricing (which needs the root hash when derived).
    let free = BundleManifest::new(BundleKind::Web, &entry, files, Pricing::Free)
        .map_err(|e| e.to_string())?;
    let pricing = match (paid_item, paid_from_root) {
        (Some(item), _) => Pricing::Paid { item },
        (None, true) => Pricing::paid_for_bundle(&free.root_hash()),
        (None, false) => Pricing::Free,
    };
    let manifest = BundleManifest {
        pricing: pricing.clone(),
        ..free
    };

    let policy = ServePolicy {
        isolation,
        ..ServePolicy::default()
    };
    let bundle = HostedBundle::new(manifest, policy).map_err(|e| e.to_string())?;
    let total = bundle.manifest.total_len();
    let count = bundle.manifest.files.len();

    let rail = MockPaymentRail::new();
    let mut host = WebHost::new(blobs).with_rail(rail);

    // A receipt the operator can paste, minted before the host takes the rail.
    let cookie = if let Pricing::Paid { item } = &pricing {
        let rail = MockPaymentRail::new();
        let buyer: PubKey = RawKeypairAuth::from_seed([42u8; 32]).node_pubkey();
        let receipt = rail
            .checkout_for_item(
                &buyer,
                item,
                PaymentSplit {
                    developer: Split {
                        wallet: RawKeypairAuth::from_seed([43u8; 32]).node_pubkey(),
                        amount: 500,
                    },
                    operator: None,
                    protocol_fee_bps: 0,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(&receipt).map_err(|e| e.to_string())?;
        Some(hex::encode(json))
    } else {
        None
    };

    let root = host.publish(bundle);

    println!("bundle root  {}", root.to_hex());
    println!("files        {count} ({total} bytes)");
    println!("entry        {entry}");
    println!(
        "pricing      {}",
        match &pricing {
            Pricing::Free => "free".to_string(),
            Pricing::Paid { item } => format!("paid, item = {item}"),
        }
    );
    println!(
        "isolation    {}",
        match isolation {
            CrossOriginIsolation::Enabled =>
                "COOP same-origin + COEP require-corp (SharedArrayBuffer available; \
                 cross-origin subresources BLOCKED)",
            CrossOriginIsolation::Disabled =>
                "off (cross-origin subresources allowed; Godot 4 will NOT boot)",
        }
    );
    if let Some(c) = &cookie {
        println!("\npaid bundle — present a receipt as either:");
        println!("  Cookie: {RECEIPT_COOKIE}={c}");
        println!("  X-Magnetite-Receipt: {c}");
        println!("(deterministic MockPaymentRail receipt; worth nothing off this process)");
    }

    magnetite_web_host::server::serve(Arc::new(host), addr, move |bound| {
        println!("\nlistening    http://{bound}/pkg/{}/", root.to_hex());
        if !bound.ip().is_loopback() {
            println!(
                "WARNING      {bound} is not loopback and this server has no TLS. \
                 crossOriginIsolated requires a secure context, so SharedArrayBuffer \
                 will be unavailable and a Godot 4 export will not boot. Put TLS in front."
            );
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Collect bundle-relative paths under `root`, recursively.
///
/// Symlinks are skipped rather than followed: following one would let a bundle
/// directory pull in bytes from outside itself, which breaks the property that a
/// root hash describes the bundle.
fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in entries {
        let e = e.map_err(|e| e.to_string())?;
        let path = e.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            eprintln!("skipping symlink {}", path.display());
            continue;
        }
        if meta.is_dir() {
            collect(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or_else(|| format!("{} is not valid UTF-8", path.display()))?
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}
