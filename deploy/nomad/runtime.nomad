# Nomad job spec — magnetite-runtime authoritative game-server host.
# Image: ghcr.io/magnetite/runtime:latest
# Port: 9000 (WebSocket, TCP)
# Binary: /usr/local/bin/magnetite-serve
#
# Scaling strategy:
#   - count = 1 for a single-node / light deployment.
#   - Increase count manually, or use Nomad autoscaler
#     (https://developer.hashicorp.com/nomad/tools/autoscaling) with a custom
#     policy based on active_session metrics from Prometheus.
#   - Each runtime allocation can serve multiple SharedRoom sessions or a single
#     Dedicated session (topology set at provision time).
#   - See docs/self-hosting/deploy.md §scaling for the full fleet design.

job "magnetite-runtime" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  # Graceful replacement on update: bring up new allocation, wait until healthy,
  # then stop old one. Existing WebSocket sessions on the old allocation will
  # drop — implement graceful session migration for zero-drop upgrades.
  update {
    max_parallel      = 1
    min_healthy_time  = "15s"
    healthy_deadline  = "5m"
    progress_deadline = "10m"
    auto_revert       = true
  }

  group "runtime" {
    count = 1

    network {
      port "ws" {
        static = 9000
        to     = 9000
      }
    }

    service {
      name     = "magnetite-runtime"
      port     = "ws"
      provider = "consul"

      # TCP check: port is bound ⟹ process is up (no HTTP health endpoint).
      check {
        type     = "tcp"
        interval = "20s"
        timeout  = "5s"
      }
    }

    task "runtime" {
      driver = "docker"

      config {
        image = "ghcr.io/magnetite/runtime:latest"
        ports = ["ws"]
        args  = ["--host", "0.0.0.0", "--port", "9000"]
      }

      template {
        data        = <<EOT
{{- range nomadVar "magnetite/secrets" }}
{{ .Key }}={{ .Value }}
{{ end -}}
EOT
        destination = "secrets/magnetite.env"
        env         = true
      }

      env {
        RUNTIME_HOST      = "0.0.0.0"
        RUNTIME_PORT      = "9000"
        RUNTIME_WORKERS   = "0"
        RUST_LOG          = "info"
        # Backend URL for provisioning poll + artifact resolution.
        MAGNETITE_API_URL = "http://{{ env "NOMAD_UPSTREAM_ADDR_backend" }}"
      }

      resources {
        cpu    = 1000
        memory = 1024
      }

      # Long termination timeout: give in-flight sessions time to finish.
      kill_timeout = "120s"

      restart {
        attempts = 3
        interval = "5m"
        delay    = "15s"
        mode     = "delay"
      }
    }
  }
}
