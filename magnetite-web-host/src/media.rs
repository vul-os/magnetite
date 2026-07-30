//! `Content-Type` and `Content-Encoding` for a bundle path.
//!
//! This module is the fix for the third bullet of ALIGNMENT.md §5:
//!
//! > **Compression.** Godot and Unity ship `.wasm.br` / `.pck.gz`; without
//! > correct `Content-Encoding` they fail silently or download uncompressed.
//!
//! The rule that makes it work, and the one everybody gets wrong: for a URL
//! ending in `.br`, the compression suffix is a **transfer** property and the
//! remaining extension is the **content** property. `index.wasm.br` is
//!
//! ```text
//! Content-Type:     application/wasm      <- what it IS
//! Content-Encoding: br                    <- how it is being sent
//! ```
//!
//! and *not* `Content-Type: application/x-brotli`, and *not*
//! `application/octet-stream`. Get it wrong in either direction and you get one
//! of the two silent failures:
//!
//! * Omit `Content-Encoding: br` and the browser hands compressed bytes to
//!   `WebAssembly.instantiateStreaming`, which rejects with a magic-number
//!   error — or, for a `.pck`, Godot reads garbage. Unity's loader requests
//!   `Build/*.wasm.br` by that literal URL and depends entirely on the server
//!   to declare the encoding.
//! * Send `Content-Type: application/octet-stream` on a `.wasm` and
//!   `instantiateStreaming` refuses it before decoding anything: it requires
//!   `application/wasm`.
//!
//! Both failures look like "the game just doesn't start".

/// A content coding applied to stored bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// Stored as-is; no `Content-Encoding` header.
    Identity,
    /// Brotli. `Content-Encoding: br`.
    Brotli,
    /// gzip. `Content-Encoding: gzip`.
    Gzip,
}

impl Encoding {
    /// The `Content-Encoding` token, or `None` for identity.
    pub fn header_value(&self) -> Option<&'static str> {
        match self {
            Self::Identity => None,
            Self::Brotli => Some("br"),
            Self::Gzip => Some("gzip"),
        }
    }

    /// The filename suffix that denotes this encoding, e.g. `.br`.
    pub fn suffix(&self) -> Option<&'static str> {
        match self {
            Self::Identity => None,
            Self::Brotli => Some(".br"),
            Self::Gzip => Some(".gz"),
        }
    }

    /// The precompressed variants a negotiating server may substitute, best
    /// first. Brotli beats gzip on every asset a game engine ships.
    pub const PRECOMPRESSED: [Encoding; 2] = [Encoding::Brotli, Encoding::Gzip];
}

/// Split a stored path into the encoding its suffix declares and the logical
/// path underneath.
///
/// ```text
/// "index.wasm.br"  -> (Brotli,   "index.wasm")
/// "index.pck.gz"   -> (Gzip,     "index.pck")
/// "index.wasm"     -> (Identity, "index.wasm")
/// ```
///
/// A bare `.br`/`.gz` with nothing underneath (`"data.br"` is fine, `".br"` is
/// not) stays `Identity`: guessing a content type for an empty stem would be
/// inventing information.
pub fn split_encoding(path: &str) -> (Encoding, &str) {
    for enc in Encoding::PRECOMPRESSED {
        let Some(suffix) = enc.suffix() else { continue };
        if let Some(stem) = path.strip_suffix(suffix) {
            if !stem.is_empty() && !stem.ends_with('/') {
                return (enc, stem);
            }
        }
    }
    (Encoding::Identity, path)
}

/// `Content-Type` for a stored path, looking through any `.br`/`.gz` suffix.
///
/// Unknown extensions get `application/octet-stream` — the honest answer, and
/// safe because every response also carries `X-Content-Type-Options: nosniff`,
/// so the browser will not upgrade a guess into script execution.
pub fn content_type(path: &str) -> &'static str {
    let (_, logical) = split_encoding(path);
    let ext = logical
        .rsplit_once('.')
        .map(|(_, e)| e)
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        // The two that must be exact or wasm instantiation fails.
        "wasm" => "application/wasm",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",

        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "txt" | "md" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "wat" => "text/plain; charset=utf-8",

        // Godot's resource pack and Unity's data blob have no registered type.
        "pck" | "data" | "bin" | "mem" | "unityweb" | "bundle" | "res" => {
            "application/octet-stream"
        }

        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "ktx2" => "image/ktx2",
        "basis" => "application/octet-stream",

        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "video/webm",

        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",

        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "webmanifest" => "application/manifest+json",
        "wasm.br" => "application/wasm", // defensive; split_encoding handles it

        _ => "application/octet-stream",
    }
}

/// Whether a client's `Accept-Encoding` header accepts `enc`.
///
/// Deliberately narrow: it looks for the exact token and refuses it if that
/// token carries `q=0`. It does **not** implement full RFC 9110 q-value
/// ordering, because the only decision made from it is "may I substitute a
/// precompressed variant", and the fallback — send the identity bytes — is
/// always correct. Being wrong here costs bandwidth, never correctness, and a
/// simple parser that is obviously conservative beats a clever one that might
/// substitute an encoding the client cannot read.
pub fn accepts(accept_encoding: &str, enc: Encoding) -> bool {
    let Some(token) = enc.header_value() else {
        return true; // identity is always acceptable
    };
    for part in accept_encoding.split(',') {
        let part = part.trim();
        let (name, params) = match part.split_once(';') {
            Some((n, p)) => (n.trim(), p.trim()),
            None => (part, ""),
        };
        if !name.eq_ignore_ascii_case(token) && name != "*" {
            continue;
        }
        // `q=0` means "explicitly do not send me this".
        let refused = params
            .split(';')
            .filter_map(|p| p.trim().strip_prefix("q="))
            .any(|q| {
                q.trim_start_matches('0')
                    .trim_start_matches('.')
                    .trim_matches('0')
                    .is_empty()
            });
        if !refused {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact pair ALIGNMENT.md §5 calls out. This is the test that would
    /// have caught the "downloads uncompressed / fails silently" bug.
    #[test]
    fn precompressed_wasm_is_application_wasm_plus_br() {
        assert_eq!(
            split_encoding("index.wasm.br"),
            (Encoding::Brotli, "index.wasm")
        );
        assert_eq!(content_type("index.wasm.br"), "application/wasm");
        assert_eq!(Encoding::Brotli.header_value(), Some("br"));

        assert_eq!(
            split_encoding("index.pck.gz"),
            (Encoding::Gzip, "index.pck")
        );
        assert_eq!(content_type("index.pck.gz"), "application/octet-stream");
        assert_eq!(Encoding::Gzip.header_value(), Some("gzip"));

        // Unity's framework JS.
        assert_eq!(
            content_type("Build/g.framework.js.br"),
            "text/javascript; charset=utf-8"
        );
        // Unity's data blob.
        assert_eq!(content_type("Build/g.data.br"), "application/octet-stream");
    }

    #[test]
    fn identity_paths_are_unchanged() {
        assert_eq!(
            split_encoding("index.wasm"),
            (Encoding::Identity, "index.wasm")
        );
        assert_eq!(content_type("index.wasm"), "application/wasm");
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("index.js"), "text/javascript; charset=utf-8");
        assert_eq!(
            content_type("index.audio.worklet.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("mystery"), "application/octet-stream");
        // A bare suffix has no stem to type, so it is not treated as encoded.
        assert_eq!(split_encoding(".br"), (Encoding::Identity, ".br"));
    }

    #[test]
    fn accept_encoding_is_conservative() {
        assert!(accepts("gzip, deflate, br", Encoding::Brotli));
        assert!(accepts("gzip, deflate, br", Encoding::Gzip));
        assert!(!accepts("gzip, deflate", Encoding::Brotli));
        assert!(!accepts("", Encoding::Brotli));
        assert!(accepts("*", Encoding::Brotli));
        assert!(
            accepts("BR", Encoding::Brotli),
            "token match is case-insensitive"
        );
        // Identity needs no permission.
        assert!(accepts("", Encoding::Identity));
        // An explicit refusal is honoured.
        assert!(!accepts("br;q=0", Encoding::Brotli));
        assert!(!accepts("gzip, br;q=0.0", Encoding::Brotli));
        assert!(accepts("br;q=0.5", Encoding::Brotli));
    }
}
