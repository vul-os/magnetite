# Live Streaming with MediaMTX (optional)

[MediaMTX](https://github.com/bluenviron/mediamtx) is **one optional** media plane
for live-stream broadcast and watch. It is not required: the backend has no
dependency on it, `MEDIA_SERVER_BASE_URL` is empty by default, and in
`docker-compose.yml` the `mediamtx` service sits behind the `media` compose
profile, so it only starts when you ask for it:

```bash
docker compose --profile media up
```

Media is **per-operator**. Every operator runs their own media server — there is no
single global one, and a stream/room record carries its own `media_host`, which
always wins over `MEDIA_SERVER_BASE_URL`.

This guide covers the full flow from a streamer going live via RTMP to a viewer
watching via HLS, plus the relevant configuration knobs.

---

## Alternatives: pluggable comms providers

Streaming, voice and video sit behind the `CommsProvider` seam, selected with
`COMMS_PROVIDER`. Providers that bring their own media plane need no MediaMTX at
all:

| `COMMS_PROVIDER` | Media plane |
|------------------|-------------|
| `builtin` (default) | The demoted in-house stack — optionally MediaMTX for HLS |
| `jitsi` | Jitsi's own SFU |
| `livekit` | LiveKit's own SFU |
| `owncast` | Owncast's own live/VOD server |
| `matrix` | Text / DMs / presence (no media plane of its own) |

A provider whose service is not configured falls back to `builtin` with a warning.
Only choose the MediaMTX path below if you are running `builtin` and want HLS
broadcast on this node.

---

## Architecture overview

```
Streamer (OBS / FFmpeg)
        │
        │  RTMP  :1935
        ▼
  ┌─────────────────┐
  │    MediaMTX     │  ← bluenviron/mediamtx container
  │                 │
  │  RTMP  :1935    │  ingest
  │  RTSP  :8554    │  re-stream (internal)
  │  HLS   :8888    │  watch (browser / curl)
  │  WHIP  :8889    │  WebRTC ingest (browser camera)
  └────────┬────────┘
           │  http://mediamtx:8888  (intra-compose)
           ▼
   Magnetite Backend (Axum)
   MEDIA_SERVER_BASE_URL=http://mediamtx:8888
           │
           │  /api/v1/streams/:id/hls  (redirect / proxy)
           ▼
      Browser player
      (HLS.js / native <video>)
```

When the `media` profile is up, MediaMTX and the backend communicate entirely
within the Docker Compose network (`magnetite` bridge). Only the ports listed in
`docker-compose.yml` are exposed to the host. The backend declares no
`depends_on` for `mediamtx` — it starts and runs fine without it.

---

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 1935 | TCP | RTMP ingest — OBS, FFmpeg, streaming software |
| 8888 | TCP | HLS watch + MediaMTX REST API |
| 8889 | TCP | WebRTC / WHIP ingest (browser camera) |
| 8554 | TCP | RTSP (internal re-stream) |

All ports can be overridden in `.env`:

```dotenv
MEDIAMTX_RTMP_PORT=1935
MEDIAMTX_HLS_PORT=8888
MEDIAMTX_WEBRTC_PORT=8889
MEDIAMTX_RTSP_PORT=8554
```

---

## Go-Live → RTMP ingest → HLS watch flow

### Step 1 — Streamer goes live (RTMP ingest)

The streamer (creator) pushes an RTMP stream to MediaMTX. The path segment
after the port becomes the **stream path** used for all subsequent references.

Using OBS:
1. Open **Settings → Stream**.
2. Set **Service** to `Custom`.
3. Set **Server** to `rtmp://localhost:1935/live`.
4. Set **Stream Key** to any identifier, e.g. `my-stream-key`.
5. Click **Start Streaming**.

Using FFmpeg (testing):
```bash
ffmpeg -re -i input.mp4 \
  -c:v libx264 -preset veryfast -b:v 2000k \
  -c:a aac -b:a 128k \
  -f flv rtmp://localhost:1935/live/my-stream-key
```

MediaMTX accepts the RTMP connection and immediately begins producing HLS
segments at the configured `hlsSegmentDuration` (default: 2 s).

### Step 2 — Backend wires the stream URL

When the streamer clicks **Go Live** in the Magnetite UI, the frontend calls:

```
POST /api/v1/streams
{
  "game_id": "<uuid>",
  "title": "My live game session",
  "rtmp_key": "my-stream-key"
}
```

The backend stores the stream record with `rtmp_key = "my-stream-key"` and
constructs the HLS manifest URL as:

```
{MEDIA_SERVER_BASE_URL}/live/{rtmp_key}/index.m3u8
→ http://mediamtx:8888/live/my-stream-key/index.m3u8
```

This URL is served back to viewers via:

```
GET /api/v1/streams/:id/hls
→ 302 redirect to http://mediamtx:8888/live/my-stream-key/index.m3u8
```

If `MEDIA_SERVER_BASE_URL` is not set, this endpoint returns **HTTP 503**
(`MediaServerUnconfigured`).

### Step 3 — Viewer watches via HLS

The viewer's browser loads the HLS manifest directly from MediaMTX (after the
302 redirect) using an HLS player:

```
http://localhost:8888/live/my-stream-key/index.m3u8
```

HLS.js (bundled with the Magnetite frontend) handles adaptive bitrate and
segment loading. Latency at the default `hlsSegmentDuration: 2s` is typically
6–10 s (3× segment duration + player buffer).

---

## Configuration

The default config is at `config/mediamtx.yml` (mounted into the container at
`/mediamtx.yml`). Key settings:

```yaml
# config/mediamtx.yml

logLevel: info            # debug | info | warn | error
hlsSegmentDuration: 2s    # lower = lower latency
hlsSegmentCount: 7        # segments kept in memory

rtmp: yes
hls: yes
webrtc: yes

paths:
  all_others:             # allow all paths without auth in dev
  live:                   # named path for OBS ingest
```

### Environment variable overrides

MediaMTX reads env vars prefixed `MTX_` as overrides for any config key:

```dotenv
MTX_LOGLEVEL=debug
MTX_HLSSEGMENTDURATION=1s
MTX_HLSSEGMENTCOUNT=5
```

Set these in `.env` or in the `mediamtx.environment` block of
`docker-compose.override.yml`.

---

## RTMP egress to Twitch / YouTube

To re-stream to an external platform, add a `runOnPublish` hook in
`config/mediamtx.yml`:

```yaml
paths:
  live:
    runOnPublish: >
      ffmpeg -i rtsp://127.0.0.1:8554/$MTX_PATH
      -c:v libx264 -preset veryfast -b:v 3000k
      -c:a aac -b:a 128k
      -f flv rtmp://live.twitch.tv/app/YOUR_TWITCH_STREAM_KEY
    runOnPublishRestart: yes
```

Replace `YOUR_TWITCH_STREAM_KEY` with the key from your Twitch dashboard. For
YouTube Live use `rtmp://a.rtmp.youtube.com/live2/YOUR_KEY`.

`$MTX_PATH` is the stream path variable injected by MediaMTX (e.g. `live`).

---

## WebRTC / WHIP ingest (browser camera)

MediaMTX supports the WHIP (WebRTC-HTTP Ingest Protocol) draft. Browser-based
streamers can push a camera feed directly without OBS:

```
WHIP endpoint: http://localhost:8889/<stream-path>/whip
```

The Magnetite frontend Go-Live page uses this when the user grants camera
access. The WHIP negotiation happens over HTTP POST; media flows over UDP.

---

## Verifying MediaMTX is running

```bash
# Started only with the media profile:
#   docker compose --profile media up -d

# REST API — returns global config JSON
curl -s http://localhost:8888/v3/config/global/get | jq '.loglevel'
# → "info"

# List active paths
curl -s http://localhost:8888/v3/paths/list | jq '.items[].name'

# Check container health
docker compose ps mediamtx
```

---

## Troubleshooting

### OBS fails to connect

- Confirm the `mediamtx` container is running: `docker compose ps mediamtx`. If it
  is missing entirely, you did not start the `media` profile —
  `docker compose --profile media up -d mediamtx`.
- Check port 1935 is not already used: `lsof -i :1935`
- Try `rtmp://127.0.0.1:1935/live` (explicit IPv4 if `localhost` resolves IPv6)

### HLS player shows "manifest not found"

- The stream may not be active. Verify OBS is streaming and MediaMTX shows the
  path: `curl http://localhost:8888/v3/paths/list | jq '.items[].name'`
- Confirm `MEDIA_SERVER_BASE_URL=http://mediamtx:8888` is set in the backend
  environment (inside compose it uses the service name, not `localhost`).

### Backend returns HTTP 503 for `/streams/:id/hls`

`MEDIA_SERVER_BASE_URL` is not set or is empty — this is the default, and it is
the correct state if you are not running a media server. To enable HLS watch on
this node, start the `media` profile and set it in `.env`:

```dotenv
MEDIA_SERVER_BASE_URL=http://mediamtx:8888
```

Then restart the backend: `docker compose restart backend`.

---

## Production notes

- Run MediaMTX on a **dedicated machine or VM** for high-concurrency streams;
  HLS segment serving at scale can saturate a shared CPU.
- Place a CDN (Cloudflare, CloudFront) in front of the HLS endpoint to cache
  segments globally and reduce origin load.
- Add per-path `publishUser` / `publishPass` in `mediamtx.yml` to prevent
  unauthorized RTMP ingest in production.
- See [External Dependencies](./external-dependencies.md) for the full picture
  on what MediaMTX enables vs. what the absent behaviour is.
