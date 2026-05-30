//! Platform streaming surface — go-live, spectate, and viewer management for
//! Magnetite's in-platform broadcasting system.
//!
//! # Overview
//!
//! Magnetite supports two complementary streaming modes:
//!
//! | Mode | Description |
//! |---|---|
//! | **In-platform** | The broadcaster captures screen / game output and the
//! |                 | platform distributes it over HLS to in-platform viewers.
//! |                 | Signaling uses the existing WebSocket layer; heavy media
//! |                 | transcoding is the documented SFU/CDN scale path. |
//! | **External RTMP** | The platform forwards the RTMP stream to an external
//! |                   | destination (Twitch, YouTube, …).  The broadcaster
//! |                   | receives an `rtmp_key` and the platform acts as a relay.
//! |                   | Full SFU / media-server support (LiveKit/mediasoup) is
//! |                   | the documented production scale path. |
//!
//! # Wire protocol
//!
//! All messages travel over the platform WebSocket connection (the same `ws/`
//! layer used by comms and voice).  The SDK serialises them as JSON.
//!
//! ```text
//! Broadcaster (in-game SDK)             Magnetite backend (Axum WS)
//!   │                                            │
//!   │── ClientStreamMessage::GoLive ────────────>│
//!   │<─ ServerStreamMessage::StreamStarted ───────│  (stream_id + rtmp_key)
//!   │                                            │
//!   │── ClientStreamMessage::UpdateTitle ────────>│
//!   │<─ ServerStreamMessage::StreamUpdated ───────│
//!   │                                            │
//!   │── ClientStreamMessage::EndStream ──────────>│
//!   │<─ ServerStreamMessage::StreamEnded ─────────│
//!
//! Spectator (in-game SDK)               Magnetite backend (Axum WS)
//!   │                                            │
//!   │── ClientStreamMessage::Watch ─────────────>│
//!   │<─ ServerStreamMessage::WatchReady ──────────│  (hls_url + viewer_count)
//!   │                                            │
//!   │<─ ServerStreamMessage::ViewerCountUpdated ──│  (pushed every N seconds)
//!   │                                            │
//!   │── ClientStreamMessage::StopWatching ───────>│
//!   │<─ ServerStreamMessage::WatchStopped ────────│
//! ```
//!
//! # Example — going live
//!
//! ```rust
//! use magnetite_sdk::platform::streaming::{
//!     StreamClient, StreamConfig, GoLiveRequest, ExternalRtmpTarget,
//! };
//!
//! let mut client = StreamClient::new(StreamConfig {
//!     user_id: "u-broadcaster".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! let req = GoLiveRequest {
//!     title: "FPS speedrun attempt".to_string(),
//!     game_id: Some("fps-starter".to_string()),
//!     community_id: None,
//!     channel_id: None,
//!     external_rtmp: Some(ExternalRtmpTarget {
//!         platform: "twitch".to_string(),
//!         rtmp_url: "rtmp://live.twitch.tv/live".to_string(),
//!     }),
//! };
//!
//! let msg = client.go_live(req.clone());
//! use magnetite_sdk::platform::streaming::ClientStreamMessage;
//! assert!(matches!(msg, ClientStreamMessage::GoLive { .. }));
//! assert!(client.is_live());
//! ```
//!
//! # Example — watching a stream
//!
//! ```rust
//! use magnetite_sdk::platform::streaming::{
//!     StreamClient, StreamConfig, ClientStreamMessage,
//! };
//!
//! let mut client = StreamClient::new(StreamConfig {
//!     user_id: "u-spectator".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! let msg = client.watch("stream-abc");
//! assert!(matches!(msg, ClientStreamMessage::Watch { .. }));
//! assert!(client.is_watching("stream-abc"));
//! ```

use serde::{Deserialize, Serialize};

use super::UserId;

// ---------------------------------------------------------------------------
// Shared primitive types
// ---------------------------------------------------------------------------

/// Opaque identifier for a live stream session.
pub type StreamId = String;

/// Opaque identifier for a community (re-exported alias).
pub type CommunityId = String;

/// Opaque identifier for a channel within a community.
pub type ChannelId = String;

/// Unix milliseconds timestamp.
pub type TimestampMs = u64;

// ---------------------------------------------------------------------------
// Stream status
// ---------------------------------------------------------------------------

/// Current lifecycle status of a stream.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::StreamStatus;
///
/// let s = StreamStatus::Live;
/// let json = serde_json::to_string(&s).unwrap();
/// let back: StreamStatus = serde_json::from_str(&json).unwrap();
/// assert_eq!(s, back);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamStatus {
    /// Stream is not yet started or has been ended.
    Offline,
    /// Stream is currently live and accepting viewers.
    Live,
    /// Stream has ended (recordings may still be available).
    Ended,
}

// ---------------------------------------------------------------------------
// Metadata types
// ---------------------------------------------------------------------------

/// Summary information about a live stream — used in browse listings and the
/// spectator join flow.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::{StreamInfo, StreamStatus};
///
/// let info = StreamInfo {
///     stream_id: "stream-001".to_string(),
///     streamer_id: "u-caster".to_string(),
///     streamer_name: "SpeedyCaster".to_string(),
///     title: "World record attempt".to_string(),
///     game_id: Some("motorsport-starter".to_string()),
///     community_id: None,
///     channel_id: None,
///     status: StreamStatus::Live,
///     viewer_count: 42,
///     hls_url: Some("https://cdn.magnetite.gg/hls/stream-001.m3u8".to_string()),
///     started_at_ms: Some(1_700_000_000_000),
/// };
/// assert_eq!(info.viewer_count, 42);
/// let json = serde_json::to_string(&info).unwrap();
/// let back: StreamInfo = serde_json::from_str(&json).unwrap();
/// assert_eq!(info, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Unique stream session ID.
    pub stream_id: StreamId,
    /// User ID of the broadcaster.
    pub streamer_id: UserId,
    /// Display name of the broadcaster at stream start time.
    pub streamer_name: String,
    /// Stream title set by the broadcaster.
    pub title: String,
    /// The game being played (if any).
    pub game_id: Option<String>,
    /// The community this stream belongs to (if any).
    pub community_id: Option<CommunityId>,
    /// The specific channel this stream is broadcasting to (if any).
    pub channel_id: Option<ChannelId>,
    /// Current stream lifecycle status.
    pub status: StreamStatus,
    /// Number of current viewers.
    pub viewer_count: u32,
    /// HLS playlist URL for in-platform viewing; populated once the stream
    /// is ingested and transcoded.
    pub hls_url: Option<String>,
    /// Wall-clock time the stream went live (Unix milliseconds).
    pub started_at_ms: Option<TimestampMs>,
}

/// Configuration for an external RTMP destination.
///
/// The platform acts as an RTMP relay — the broadcaster's media is forwarded
/// to `rtmp_url` using the provided `stream_key`.  Full media-server support
/// (LiveKit / mediasoup) is the documented production scale path for high
/// concurrency.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::ExternalRtmpTarget;
///
/// let target = ExternalRtmpTarget {
///     platform: "twitch".to_string(),
///     rtmp_url: "rtmp://live.twitch.tv/live".to_string(),
/// };
/// let json = serde_json::to_string(&target).unwrap();
/// let back: ExternalRtmpTarget = serde_json::from_str(&json).unwrap();
/// assert_eq!(target.platform, back.platform);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalRtmpTarget {
    /// Human-readable platform label (e.g. `"twitch"`, `"youtube"`).
    pub platform: String,
    /// The RTMP ingest URL for the external platform.
    pub rtmp_url: String,
}

/// A request to start a new live stream.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::GoLiveRequest;
///
/// let req = GoLiveRequest {
///     title: "Ranked match commentary".to_string(),
///     game_id: Some("fps-starter".to_string()),
///     community_id: Some("community-fps".to_string()),
///     channel_id: None,
///     external_rtmp: None,
/// };
/// assert_eq!(req.title, "Ranked match commentary");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoLiveRequest {
    /// Stream title displayed to viewers.
    pub title: String,
    /// The game being played (for discoverability and categorisation).
    pub game_id: Option<String>,
    /// The community this stream should appear in.
    pub community_id: Option<CommunityId>,
    /// The channel within the community to broadcast to.
    pub channel_id: Option<ChannelId>,
    /// Optional external RTMP forward target (Twitch / YouTube / …).
    pub external_rtmp: Option<ExternalRtmpTarget>,
}

// ---------------------------------------------------------------------------
// Client → Platform messages
// ---------------------------------------------------------------------------

/// Messages sent **from** in-game Rust code **to** the Magnetite platform
/// for streaming operations.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::{ClientStreamMessage, GoLiveRequest};
///
/// let msg = ClientStreamMessage::GoLive {
///     request: GoLiveRequest {
///         title: "go live test".to_string(),
///         game_id: None,
///         community_id: None,
///         channel_id: None,
///         external_rtmp: None,
///     },
/// };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ClientStreamMessage = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, ClientStreamMessage::GoLive { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientStreamMessage {
    // -- Broadcaster messages --
    /// Start a new live stream.
    GoLive {
        /// Stream configuration.
        request: GoLiveRequest,
    },

    /// Update the title of an in-progress stream.
    UpdateTitle {
        /// The stream session to update.
        stream_id: StreamId,
        /// New title.
        title: String,
    },

    /// Gracefully end the current live stream.
    EndStream {
        /// The stream session to end.
        stream_id: StreamId,
    },

    // -- Spectator messages --
    /// Join as a viewer for the given stream.
    Watch {
        /// The stream session to watch.
        stream_id: StreamId,
    },

    /// Stop watching a stream.
    StopWatching {
        /// The stream session to leave.
        stream_id: StreamId,
    },

    // -- Discovery messages --
    /// Request a paginated list of currently live streams.
    ListStreams {
        /// Filter by game ID (optional).
        game_id: Option<String>,
        /// Filter by community ID (optional).
        community_id: Option<CommunityId>,
        /// Maximum number of results to return (default 20).
        limit: Option<u32>,
        /// Pagination offset.
        offset: Option<u32>,
    },

    /// Request info about a specific stream.
    GetStream {
        /// The stream session to query.
        stream_id: StreamId,
    },
}

// ---------------------------------------------------------------------------
// Platform → Client messages
// ---------------------------------------------------------------------------

/// Messages sent **from** the Magnetite platform **to** in-game Rust code
/// for streaming events.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::ServerStreamMessage;
///
/// let msg = ServerStreamMessage::StreamEnded {
///     stream_id: "s-001".to_string(),
/// };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ServerStreamMessage = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, ServerStreamMessage::StreamEnded { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerStreamMessage {
    // -- Broadcaster events --
    /// The stream is live.  Contains the RTMP ingest key the broadcaster
    /// should push to, and the HLS URL for in-platform viewers.
    StreamStarted {
        /// The new stream session ID.
        stream_id: StreamId,
        /// RTMP ingest key — push to
        /// `rtmp://ingest.magnetite.gg/live/<rtmp_key>` to start ingestion.
        rtmp_key: String,
        /// HLS playlist URL (may not be populated immediately; retry once
        /// transcoding has produced the first segment).
        hls_url: Option<String>,
    },

    /// Stream metadata was updated successfully.
    StreamUpdated {
        /// The updated stream info.
        stream: StreamInfo,
    },

    /// The stream has ended.
    StreamEnded {
        /// The stream that ended.
        stream_id: StreamId,
    },

    // -- Spectator events --
    /// The viewer has been admitted; start playing the HLS stream.
    WatchReady {
        /// The stream being watched.
        stream_id: StreamId,
        /// HLS playlist URL for the media player.
        hls_url: String,
        /// Current viewer count.
        viewer_count: u32,
    },

    /// The viewer has stopped watching.
    WatchStopped {
        /// The stream that was left.
        stream_id: StreamId,
    },

    /// Periodic viewer-count update pushed to all watchers.
    ViewerCountUpdated {
        /// The stream this count belongs to.
        stream_id: StreamId,
        /// New viewer count.
        viewer_count: u32,
    },

    // -- Discovery responses --
    /// Response to [`ClientStreamMessage::ListStreams`].
    StreamList {
        /// Matching streams.
        streams: Vec<StreamInfo>,
        /// Total number of streams matching the filter (for pagination).
        total: u32,
    },

    /// Response to [`ClientStreamMessage::GetStream`].
    StreamDetail(StreamInfo),

    // -- Error --
    /// An error from the streaming layer.
    Error {
        /// Machine-readable error code.
        code: StreamErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes for streaming operations.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::StreamErrorCode;
///
/// let code = StreamErrorCode::AlreadyLive;
/// let json = serde_json::to_string(&code).unwrap();
/// let back: StreamErrorCode = serde_json::from_str(&json).unwrap();
/// assert_eq!(code, back);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamErrorCode {
    /// The request requires authentication.
    Unauthorized,
    /// The broadcaster is already streaming.
    AlreadyLive,
    /// The stream was not found.
    NotFound,
    /// The stream has already ended.
    AlreadyEnded,
    /// The broadcaster is not currently live.
    NotLive,
    /// The operation is not permitted (e.g. modifying another user's stream).
    Forbidden,
    /// The platform encountered an internal error.
    Internal,
    /// The request was malformed.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Streaming client configuration
// ---------------------------------------------------------------------------

/// Configuration for the in-game streaming client.
///
/// ```rust
/// use magnetite_sdk::platform::streaming::StreamConfig;
///
/// let cfg = StreamConfig {
///     user_id: "u-caster".to_string(),
///     auth_token: "jwt-here".to_string(),
/// };
/// assert_eq!(cfg.user_id, "u-caster");
/// ```
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Platform user ID (from the identity/auth service).
    pub user_id: UserId,
    /// JWT / session token for authenticating requests.
    pub auth_token: String,
}

// ---------------------------------------------------------------------------
// Streaming client state machine
// ---------------------------------------------------------------------------

/// Typed, stateful in-game streaming client.
///
/// Manages local broadcaster / spectator state so game code can query it
/// without parsing raw messages.
///
/// **No I/O is performed** — the caller sends the returned
/// [`ClientStreamMessage`] via the platform WebSocket and passes received
/// bytes back into [`StreamClient::handle_server_message`].
///
/// ```rust
/// use magnetite_sdk::platform::streaming::{
///     GoLiveRequest, ServerStreamMessage, StreamClient, StreamConfig,
///     StreamEvent,
/// };
///
/// let mut client = StreamClient::new(StreamConfig {
///     user_id: "u-caster".to_string(),
///     auth_token: "tok".to_string(),
/// });
///
/// // Start streaming.
/// let req = GoLiveRequest {
///     title: "Racing live".to_string(),
///     game_id: Some("motorsport-starter".to_string()),
///     community_id: None,
///     channel_id: None,
///     external_rtmp: None,
/// };
/// let msg = client.go_live(req);
/// assert!(client.is_live());
///
/// // Simulate the server acknowledging the stream start.
/// let server_msg = ServerStreamMessage::StreamStarted {
///     stream_id: "s-99".to_string(),
///     rtmp_key: "live-key-abc".to_string(),
///     hls_url: Some("https://cdn.magnetite.gg/hls/s-99.m3u8".to_string()),
/// };
/// let event = client.handle_server_message(server_msg).unwrap();
/// assert!(matches!(event, StreamEvent::StreamStarted { .. }));
/// assert_eq!(client.active_stream_id(), Some("s-99"));
/// ```
#[derive(Debug, Clone)]
pub struct StreamClient {
    config: StreamConfig,
    /// Stream ID of the current broadcast (if this client is live).
    active_stream_id: Option<StreamId>,
    /// Streams this client is currently watching.
    watching: Vec<StreamId>,
    /// Whether the local user is currently broadcasting.
    is_live: bool,
}

impl StreamClient {
    /// Create a new `StreamClient`.
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            active_stream_id: None,
            watching: Vec::new(),
            is_live: false,
        }
    }

    /// The authenticated user ID.
    pub fn user_id(&self) -> &str {
        &self.config.user_id
    }

    /// Whether the local user is currently broadcasting.
    pub fn is_live(&self) -> bool {
        self.is_live
    }

    /// The stream ID of the current broadcast, or `None` if not live.
    pub fn active_stream_id(&self) -> Option<&str> {
        self.active_stream_id.as_deref()
    }

    /// Whether the client is currently watching the given stream.
    pub fn is_watching(&self, stream_id: &str) -> bool {
        self.watching.iter().any(|s| s == stream_id)
    }

    /// Snapshot of stream IDs this client is watching.
    pub fn watching(&self) -> &[StreamId] {
        &self.watching
    }

    // -- Broadcaster operations --

    /// Build a [`ClientStreamMessage::GoLive`] and mark the client as live.
    ///
    /// Returns an error string if the client is already live.
    pub fn go_live(&mut self, request: GoLiveRequest) -> ClientStreamMessage {
        self.is_live = true;
        ClientStreamMessage::GoLive { request }
    }

    /// Build a [`ClientStreamMessage::UpdateTitle`].
    ///
    /// Returns an error if not currently live.
    pub fn update_title(&self, title: &str) -> Result<ClientStreamMessage, &'static str> {
        let stream_id = self.active_stream_id.clone().ok_or("not live")?;
        Ok(ClientStreamMessage::UpdateTitle {
            stream_id,
            title: title.to_string(),
        })
    }

    /// Build a [`ClientStreamMessage::EndStream`] and clear live state.
    ///
    /// Returns an error if not currently live.
    pub fn end_stream(&mut self) -> Result<ClientStreamMessage, &'static str> {
        let stream_id = self.active_stream_id.clone().ok_or("not live")?;
        self.is_live = false;
        self.active_stream_id = None;
        Ok(ClientStreamMessage::EndStream { stream_id })
    }

    // -- Spectator operations --

    /// Build a [`ClientStreamMessage::Watch`] and record the subscription.
    ///
    /// Returns an error if already watching this stream.
    pub fn watch(&mut self, stream_id: &str) -> ClientStreamMessage {
        if !self.watching.iter().any(|s| s == stream_id) {
            self.watching.push(stream_id.to_string());
        }
        ClientStreamMessage::Watch {
            stream_id: stream_id.to_string(),
        }
    }

    /// Build a [`ClientStreamMessage::StopWatching`] and clear the subscription.
    ///
    /// Returns an error if not watching this stream.
    pub fn stop_watching(&mut self, stream_id: &str) -> Result<ClientStreamMessage, &'static str> {
        let pos = self
            .watching
            .iter()
            .position(|s| s == stream_id)
            .ok_or("not watching this stream")?;
        self.watching.swap_remove(pos);
        Ok(ClientStreamMessage::StopWatching {
            stream_id: stream_id.to_string(),
        })
    }

    // -- Discovery operations --

    /// Build a [`ClientStreamMessage::ListStreams`] request.
    pub fn list_streams(
        &self,
        game_id: Option<String>,
        community_id: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientStreamMessage {
        ClientStreamMessage::ListStreams {
            game_id,
            community_id,
            limit,
            offset,
        }
    }

    /// Build a [`ClientStreamMessage::GetStream`] request.
    pub fn get_stream(&self, stream_id: &str) -> ClientStreamMessage {
        ClientStreamMessage::GetStream {
            stream_id: stream_id.to_string(),
        }
    }

    // -- Inbound message dispatch --

    /// Process a [`ServerStreamMessage`] received from the platform and return
    /// the corresponding [`StreamEvent`].
    ///
    /// Also updates local state (e.g. records the `stream_id` on
    /// `StreamStarted`, clears live state on `StreamEnded`).
    pub fn handle_server_message(
        &mut self,
        msg: ServerStreamMessage,
    ) -> Result<StreamEvent, &'static str> {
        match msg {
            ServerStreamMessage::StreamStarted {
                stream_id,
                rtmp_key,
                hls_url,
            } => {
                self.active_stream_id = Some(stream_id.clone());
                self.is_live = true;
                Ok(StreamEvent::StreamStarted {
                    stream_id,
                    rtmp_key,
                    hls_url,
                })
            }

            ServerStreamMessage::StreamUpdated { stream } => {
                Ok(StreamEvent::StreamUpdated { stream })
            }

            ServerStreamMessage::StreamEnded { stream_id } => {
                self.is_live = false;
                self.active_stream_id = None;
                Ok(StreamEvent::StreamEnded { stream_id })
            }

            ServerStreamMessage::WatchReady {
                stream_id,
                hls_url,
                viewer_count,
            } => Ok(StreamEvent::WatchReady {
                stream_id,
                hls_url,
                viewer_count,
            }),

            ServerStreamMessage::WatchStopped { stream_id } => {
                self.watching.retain(|s| s != &stream_id);
                Ok(StreamEvent::WatchStopped { stream_id })
            }

            ServerStreamMessage::ViewerCountUpdated {
                stream_id,
                viewer_count,
            } => Ok(StreamEvent::ViewerCountUpdated {
                stream_id,
                viewer_count,
            }),

            ServerStreamMessage::StreamList { streams, total } => {
                Ok(StreamEvent::StreamList { streams, total })
            }

            ServerStreamMessage::StreamDetail(info) => Ok(StreamEvent::StreamDetail(info)),

            ServerStreamMessage::Error { code, message } => {
                Ok(StreamEvent::Error { code, message })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stream events
// ---------------------------------------------------------------------------

/// Inbound events surfaced to game code by
/// [`StreamClient::handle_server_message`].
///
/// The game's streaming UI / overlay layer should match on these variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// The stream went live.  The broadcaster should push RTMP to the ingest
    /// endpoint using the returned `rtmp_key`.
    StreamStarted {
        /// The new stream session ID.
        stream_id: StreamId,
        /// RTMP ingest key.
        rtmp_key: String,
        /// HLS playlist URL (may be `None` until transcoding starts).
        hls_url: Option<String>,
    },

    /// Stream metadata was updated.
    StreamUpdated {
        /// Updated stream info.
        stream: StreamInfo,
    },

    /// The stream has ended (broadcaster or moderator action).
    StreamEnded {
        /// The stream session that ended.
        stream_id: StreamId,
    },

    /// The viewer is ready to watch; start the HLS player.
    WatchReady {
        /// The stream being watched.
        stream_id: StreamId,
        /// HLS playlist URL.
        hls_url: String,
        /// Current viewer count.
        viewer_count: u32,
    },

    /// The viewer stopped watching.
    WatchStopped {
        /// The stream that was left.
        stream_id: StreamId,
    },

    /// Live viewer count push from the server.
    ViewerCountUpdated {
        /// The stream.
        stream_id: StreamId,
        /// New viewer count.
        viewer_count: u32,
    },

    /// Response to a [`ClientStreamMessage::ListStreams`] request.
    StreamList {
        /// Matching streams.
        streams: Vec<StreamInfo>,
        /// Total number matching the filter.
        total: u32,
    },

    /// Response to a [`ClientStreamMessage::GetStream`] request.
    StreamDetail(StreamInfo),

    /// An error from the streaming layer.
    Error {
        /// Machine-readable error code.
        code: StreamErrorCode,
        /// Human-readable description.
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn broadcaster() -> StreamClient {
        StreamClient::new(StreamConfig {
            user_id: "u-caster".to_string(),
            auth_token: "tok-caster".to_string(),
        })
    }

    fn spectator() -> StreamClient {
        StreamClient::new(StreamConfig {
            user_id: "u-spectator".to_string(),
            auth_token: "tok-spectator".to_string(),
        })
    }

    fn sample_go_live() -> GoLiveRequest {
        GoLiveRequest {
            title: "FPS World Record Attempt".to_string(),
            game_id: Some("fps-starter".to_string()),
            community_id: None,
            channel_id: None,
            external_rtmp: Some(ExternalRtmpTarget {
                platform: "twitch".to_string(),
                rtmp_url: "rtmp://live.twitch.tv/live".to_string(),
            }),
        }
    }

    fn sample_stream_info() -> StreamInfo {
        StreamInfo {
            stream_id: "stream-001".to_string(),
            streamer_id: "u-caster".to_string(),
            streamer_name: "SpeedyCaster".to_string(),
            title: "FPS World Record Attempt".to_string(),
            game_id: Some("fps-starter".to_string()),
            community_id: None,
            channel_id: None,
            status: StreamStatus::Live,
            viewer_count: 42,
            hls_url: Some("https://cdn.magnetite.gg/hls/stream-001.m3u8".to_string()),
            started_at_ms: Some(1_700_000_000_000),
        }
    }

    // -----------------------------------------------------------------------
    // Serde roundtrip tests
    // -----------------------------------------------------------------------

    #[test]
    fn stream_status_all_variants_roundtrip() {
        let statuses = [
            StreamStatus::Offline,
            StreamStatus::Live,
            StreamStatus::Ended,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let back: StreamStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, &back);
        }
    }

    #[test]
    fn stream_info_roundtrip() {
        let info = sample_stream_info();
        let json = serde_json::to_string(&info).unwrap();
        let back: StreamInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }

    #[test]
    fn external_rtmp_target_roundtrip() {
        let target = ExternalRtmpTarget {
            platform: "youtube".to_string(),
            rtmp_url: "rtmp://a.rtmp.youtube.com/live2".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let back: ExternalRtmpTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, back);
    }

    #[test]
    fn go_live_request_roundtrip() {
        let req = sample_go_live();
        let json = serde_json::to_string(&req).unwrap();
        let back: GoLiveRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn client_stream_message_all_variants_roundtrip() {
        let msgs: Vec<ClientStreamMessage> = vec![
            ClientStreamMessage::GoLive {
                request: sample_go_live(),
            },
            ClientStreamMessage::UpdateTitle {
                stream_id: "s-1".to_string(),
                title: "New title".to_string(),
            },
            ClientStreamMessage::EndStream {
                stream_id: "s-1".to_string(),
            },
            ClientStreamMessage::Watch {
                stream_id: "s-2".to_string(),
            },
            ClientStreamMessage::StopWatching {
                stream_id: "s-2".to_string(),
            },
            ClientStreamMessage::ListStreams {
                game_id: Some("fps-starter".to_string()),
                community_id: None,
                limit: Some(10),
                offset: Some(0),
            },
            ClientStreamMessage::GetStream {
                stream_id: "s-3".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ClientStreamMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn server_stream_message_all_variants_roundtrip() {
        let msgs: Vec<ServerStreamMessage> = vec![
            ServerStreamMessage::StreamStarted {
                stream_id: "s-1".to_string(),
                rtmp_key: "key-abc".to_string(),
                hls_url: Some("https://cdn.magnetite.gg/hls/s-1.m3u8".to_string()),
            },
            ServerStreamMessage::StreamUpdated {
                stream: sample_stream_info(),
            },
            ServerStreamMessage::StreamEnded {
                stream_id: "s-1".to_string(),
            },
            ServerStreamMessage::WatchReady {
                stream_id: "s-1".to_string(),
                hls_url: "https://cdn.magnetite.gg/hls/s-1.m3u8".to_string(),
                viewer_count: 99,
            },
            ServerStreamMessage::WatchStopped {
                stream_id: "s-1".to_string(),
            },
            ServerStreamMessage::ViewerCountUpdated {
                stream_id: "s-1".to_string(),
                viewer_count: 150,
            },
            ServerStreamMessage::StreamList {
                streams: vec![sample_stream_info()],
                total: 1,
            },
            ServerStreamMessage::StreamDetail(sample_stream_info()),
            ServerStreamMessage::Error {
                code: StreamErrorCode::AlreadyLive,
                message: "already streaming".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ServerStreamMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn stream_error_code_all_variants_roundtrip() {
        let codes = [
            StreamErrorCode::Unauthorized,
            StreamErrorCode::AlreadyLive,
            StreamErrorCode::NotFound,
            StreamErrorCode::AlreadyEnded,
            StreamErrorCode::NotLive,
            StreamErrorCode::Forbidden,
            StreamErrorCode::Internal,
            StreamErrorCode::BadRequest,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let back: StreamErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &back);
        }
    }

    // -----------------------------------------------------------------------
    // StreamClient state machine tests
    // -----------------------------------------------------------------------

    #[test]
    fn broadcaster_initial_state() {
        let client = broadcaster();
        assert!(!client.is_live());
        assert_eq!(client.active_stream_id(), None);
        assert!(client.watching().is_empty());
        assert_eq!(client.user_id(), "u-caster");
    }

    #[test]
    fn broadcaster_go_live_sets_state() {
        let mut client = broadcaster();
        let msg = client.go_live(sample_go_live());
        assert!(client.is_live());
        assert!(matches!(msg, ClientStreamMessage::GoLive { .. }));
    }

    #[test]
    fn broadcaster_stream_started_records_stream_id() {
        let mut client = broadcaster();
        client.go_live(sample_go_live());

        let event = client
            .handle_server_message(ServerStreamMessage::StreamStarted {
                stream_id: "s-42".to_string(),
                rtmp_key: "key-xyz".to_string(),
                hls_url: None,
            })
            .unwrap();

        assert!(matches!(event, StreamEvent::StreamStarted { .. }));
        assert_eq!(client.active_stream_id(), Some("s-42"));
        assert!(client.is_live());
    }

    #[test]
    fn broadcaster_end_stream_clears_state() {
        let mut client = broadcaster();
        client.go_live(sample_go_live());
        client
            .handle_server_message(ServerStreamMessage::StreamStarted {
                stream_id: "s-42".to_string(),
                rtmp_key: "key-xyz".to_string(),
                hls_url: None,
            })
            .unwrap();

        let msg = client.end_stream().unwrap();
        assert!(!client.is_live());
        assert_eq!(client.active_stream_id(), None);
        assert!(matches!(msg, ClientStreamMessage::EndStream { .. }));
    }

    #[test]
    fn broadcaster_end_stream_fails_when_not_live() {
        let mut client = broadcaster();
        assert!(client.end_stream().is_err());
    }

    #[test]
    fn broadcaster_update_title_fails_when_not_live() {
        let client = broadcaster();
        assert!(client.update_title("new title").is_err());
    }

    #[test]
    fn broadcaster_update_title_succeeds_when_live() {
        let mut client = broadcaster();
        client.go_live(sample_go_live());
        client
            .handle_server_message(ServerStreamMessage::StreamStarted {
                stream_id: "s-10".to_string(),
                rtmp_key: "k".to_string(),
                hls_url: None,
            })
            .unwrap();
        let msg = client.update_title("Speed record attempt").unwrap();
        assert!(matches!(
            msg,
            ClientStreamMessage::UpdateTitle { title, .. } if title == "Speed record attempt"
        ));
    }

    #[test]
    fn stream_ended_server_message_clears_broadcaster_state() {
        let mut client = broadcaster();
        client.go_live(sample_go_live());
        client
            .handle_server_message(ServerStreamMessage::StreamStarted {
                stream_id: "s-99".to_string(),
                rtmp_key: "k".to_string(),
                hls_url: None,
            })
            .unwrap();

        let event = client
            .handle_server_message(ServerStreamMessage::StreamEnded {
                stream_id: "s-99".to_string(),
            })
            .unwrap();

        assert!(matches!(event, StreamEvent::StreamEnded { .. }));
        assert!(!client.is_live());
        assert_eq!(client.active_stream_id(), None);
    }

    #[test]
    fn spectator_watch_records_state() {
        let mut client = spectator();
        let msg = client.watch("stream-abc");
        assert!(client.is_watching("stream-abc"));
        assert!(matches!(
            msg,
            ClientStreamMessage::Watch { stream_id } if stream_id == "stream-abc"
        ));
    }

    #[test]
    fn spectator_watch_idempotent() {
        let mut client = spectator();
        client.watch("stream-abc");
        client.watch("stream-abc"); // second call should not duplicate
        assert_eq!(client.watching().len(), 1);
    }

    #[test]
    fn spectator_stop_watching_clears_state() {
        let mut client = spectator();
        client.watch("stream-abc");
        let msg = client.stop_watching("stream-abc").unwrap();
        assert!(!client.is_watching("stream-abc"));
        assert!(matches!(
            msg,
            ClientStreamMessage::StopWatching { stream_id } if stream_id == "stream-abc"
        ));
    }

    #[test]
    fn spectator_stop_watching_fails_for_unknown() {
        let mut client = spectator();
        assert!(client.stop_watching("no-such-stream").is_err());
    }

    #[test]
    fn spectator_watch_stopped_server_message_clears_state() {
        let mut client = spectator();
        client.watch("stream-xyz");
        let event = client
            .handle_server_message(ServerStreamMessage::WatchStopped {
                stream_id: "stream-xyz".to_string(),
            })
            .unwrap();
        assert!(matches!(event, StreamEvent::WatchStopped { .. }));
        assert!(!client.is_watching("stream-xyz"));
    }

    #[test]
    fn spectator_watch_ready_event() {
        let mut client = spectator();
        client.watch("stream-99");
        let event = client
            .handle_server_message(ServerStreamMessage::WatchReady {
                stream_id: "stream-99".to_string(),
                hls_url: "https://cdn.magnetite.gg/hls/s-99.m3u8".to_string(),
                viewer_count: 77,
            })
            .unwrap();
        assert!(matches!(
            event,
            StreamEvent::WatchReady {
                viewer_count: 77,
                ..
            }
        ));
    }

    #[test]
    fn viewer_count_updated_event() {
        let mut client = spectator();
        client.watch("stream-77");
        let event = client
            .handle_server_message(ServerStreamMessage::ViewerCountUpdated {
                stream_id: "stream-77".to_string(),
                viewer_count: 500,
            })
            .unwrap();
        assert!(matches!(
            event,
            StreamEvent::ViewerCountUpdated {
                viewer_count: 500,
                ..
            }
        ));
    }

    #[test]
    fn list_streams_message() {
        let client = spectator();
        let msg = client.list_streams(Some("fps-starter".to_string()), None, Some(5), None);
        assert!(matches!(msg, ClientStreamMessage::ListStreams { .. }));
    }

    #[test]
    fn get_stream_message() {
        let client = spectator();
        let msg = client.get_stream("stream-42");
        assert!(matches!(
            msg,
            ClientStreamMessage::GetStream { stream_id } if stream_id == "stream-42"
        ));
    }

    #[test]
    fn stream_list_event() {
        let mut client = spectator();
        let event = client
            .handle_server_message(ServerStreamMessage::StreamList {
                streams: vec![sample_stream_info()],
                total: 1,
            })
            .unwrap();
        assert!(matches!(event, StreamEvent::StreamList { total: 1, .. }));
    }

    #[test]
    fn stream_detail_event() {
        let mut client = spectator();
        let event = client
            .handle_server_message(ServerStreamMessage::StreamDetail(sample_stream_info()))
            .unwrap();
        assert!(matches!(event, StreamEvent::StreamDetail(_)));
    }

    #[test]
    fn error_event() {
        let mut client = spectator();
        let event = client
            .handle_server_message(ServerStreamMessage::Error {
                code: StreamErrorCode::NotFound,
                message: "stream not found".to_string(),
            })
            .unwrap();
        assert!(matches!(
            event,
            StreamEvent::Error {
                code: StreamErrorCode::NotFound,
                ..
            }
        ));
    }
}
