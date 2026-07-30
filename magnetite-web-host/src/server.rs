//! The hyper HTTP/1.1 binding. Translation only — no policy.
//!
//! Every status code and header this file emits was decided in
//! [`crate::respond`] or [`crate::host`]. Keeping it that way is what makes the
//! interesting behaviour testable without a socket, and it is also what lets an
//! operator put this logic behind an existing axum app or nginx instead: build
//! with `--no-default-features` and call [`WebHost::handle`] yourself.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use magnetite_seams::blobstore::BlobStore;
use magnetite_seams::payment::PaymentRail;
use tokio::net::TcpListener;

use crate::host::{RawRequest, WebHost};

/// Serve `host` on `addr` until the process ends.
///
/// Returns the bound address through `on_bound` before the accept loop starts,
/// so a caller that asked for port 0 can learn the real port. Tests use it; a
/// dev server prints it.
///
/// # TLS
///
/// There is none here, and that is a real limit rather than an omission to
/// discover later. `crossOriginIsolated` — and therefore `SharedArrayBuffer`,
/// and therefore a booting Godot 4 export — requires a **secure context**. That
/// means `https://`, with exactly one exception: `http://localhost` and
/// `127.0.0.1` count as secure, which is why plain HTTP is enough for local
/// development and for the tests in this crate.
///
/// On any other address a bundle served over plain `http://` will get its COOP
/// and COEP headers, will not be cross-origin isolated, and a Godot 4 export
/// will not boot. ALIGNMENT.md §2 blocker 1 is exactly this problem for the
/// node's WebSocket; it is the same problem here. Terminate TLS in front of this
/// (a reverse proxy or tunnel) or wait for the built-in rustls+ACME path.
pub async fn serve<B, R>(
    host: Arc<WebHost<B, R>>,
    addr: SocketAddr,
    on_bound: impl FnOnce(SocketAddr),
) -> std::io::Result<()>
where
    B: BlobStore + Send + Sync + 'static,
    R: PaymentRail + Send + Sync + 'static,
{
    let listener = TcpListener::bind(addr).await?;
    on_bound(listener.local_addr()?);

    loop {
        let (stream, _peer) = listener.accept().await?;
        let host = Arc::clone(&host);
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req: Request<Incoming>| {
                let host = Arc::clone(&host);
                async move { Ok::<_, Infallible>(dispatch(&host, req).await) }
            });
            // A dropped or malformed connection is the client's business, not an
            // event worth failing the server over.
            let _ = http1::Builder::new().serve_connection(io, svc).await;
        });
    }
}

/// Translate one hyper request into a [`WebHost`] answer.
async fn dispatch<B, R>(host: &WebHost<B, R>, req: Request<Incoming>) -> Response<Full<Bytes>>
where
    B: BlobStore,
    R: PaymentRail,
{
    let method = req.method().as_str().to_string();
    let target = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    // Non-UTF-8 header values are dropped rather than lossily converted: a
    // header we cannot read exactly is a header we must not act on, and every
    // header this crate reads is ASCII by specification.
    let owned: Vec<(String, String)> = req
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|v| (k.as_str().to_string(), v.to_string()))
        })
        .collect();
    let raw = RawRequest {
        method: &method,
        target: &target,
        headers: owned
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect(),
    };

    let out = host.handle(&raw).await;

    let mut builder = Response::builder().status(out.status);
    for (k, v) in &out.headers {
        // `Content-Length` is set from the manifest, and hyper computes its own
        // for the body it is about to write. Emitting both risks a mismatch on a
        // `HEAD` (where the manifest length is right and the body is empty), and
        // a mismatched `Content-Length` is a framing bug. So: keep ours on
        // `HEAD`, where it is the whole point, and let hyper own it otherwise.
        if k.eq_ignore_ascii_case("content-length") && !out.body.is_empty() {
            continue;
        }
        builder = builder.header(k.as_str(), v.as_str());
    }
    builder
        .body(Full::new(Bytes::from(out.body)))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Full::new(Bytes::from_static(
                    b"500 header encoding error\n",
                )))
                .expect("static 500 response is always constructible")
        })
}
