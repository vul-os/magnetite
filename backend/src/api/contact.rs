// Contact API — POST /api/v1/contact
//
// The contact handler implementation lives in `reviews.rs` (fn `submit_contact`)
// so that it can be mounted inside the reviews router without requiring a new
// `pub mod contact;` declaration in mod.rs (which is owned by a sibling agent).
//
// Once the orchestrator adds `pub mod reviews;` to mod.rs the handler is
// accessible at `crate::api::reviews::submit_contact` and the router at
// `crate::api::reviews::router`.
//
// The canonical API path is POST /api/v1/contact.  See reviews::router() for
// the mount instructions comment.
