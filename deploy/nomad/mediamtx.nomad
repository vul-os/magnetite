# Nomad job spec — MediaMTX media server.
# Ports: 8888 (HLS/API), 1935 (RTMP), 8889 (WebRTC/WHIP), 8554 (RTSP)

job "mediamtx" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  group "mediamtx" {
    count = 1

    network {
      port "hls_api" {
        static = 8888
        to     = 8888
      }
      port "rtmp" {
        static = 1935
        to     = 1935
      }
      port "webrtc" {
        static = 8889
        to     = 8889
      }
      port "rtsp" {
        static = 8554
        to     = 8554
      }
    }

    service {
      name     = "mediamtx"
      port     = "hls_api"
      provider = "consul"

      check {
        type     = "http"
        path     = "/v3/config/global/get"
        interval = "30s"
        timeout  = "10s"
      }
    }

    task "mediamtx" {
      driver = "docker"

      config {
        image = "bluenviron/mediamtx:latest"
        ports = ["hls_api", "rtmp", "webrtc", "rtsp"]
      }

      # Render the MediaMTX config file from a template.
      template {
        data        = <<EOT
logLevel: info
logDestinations: [stdout]

hls: yes
hlsAddress: :8888
hlsAlwaysRemux: no
hlsSegmentDuration: 2s
hlsSegmentMaxSize: 50MB
hlsAllowOrigin: "*"

rtmp: yes
rtmpAddress: :1935

webrtc: yes
webrtcAddress: :8889

rtsp: yes
rtspAddress: :8554

paths:
  "~^.*$":
    source: publisher
EOT
        destination = "local/mediamtx.yml"
        # Mount inside the container.
      }

      env {
        MTX_LOGLEVEL = "info"
      }

      resources {
        cpu    = 500
        memory = 512
      }

      restart {
        attempts = 3
        interval = "5m"
        delay    = "15s"
        mode     = "delay"
      }
    }
  }
}
