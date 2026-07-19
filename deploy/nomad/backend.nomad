# Nomad job spec — Magnetite backend (Axum REST API).
# Image: ghcr.io/magnetite/backend:latest
# Port: 8080
# Health: GET /health
#
# Secrets are sourced from Nomad Variables or Vault (see template block).
# The Vault integration block is shown commented-out; enable it if Vault is running.

job "backend" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  # Rolling update: bring up 1 new allocation before marking old healthy allocations
  # for stop.
  update {
    max_parallel      = 1
    min_healthy_time  = "30s"
    healthy_deadline  = "5m"
    progress_deadline = "10m"
    auto_revert       = true
    canary            = 0
  }

  group "backend" {
    count = 2

    # Anti-affinity: spread backend replicas across different nodes.
    spread {
      attribute = "${node.unique.id}"
    }

    network {
      port "http" {
        to = 8080
      }
    }

    service {
      name     = "backend"
      port     = "http"
      provider = "consul"

      check {
        type     = "http"
        path     = "/health"
        interval = "15s"
        timeout  = "10s"
      }
    }

    task "backend" {
      driver = "docker"

      config {
        image = "ghcr.io/magnetite/backend:latest"
        ports = ["http"]
      }

      # Nomad Variables-based secret injection.
      # Store secrets at: nomad var put magnetite/secrets @secrets.json
      # (or use the Vault template block below instead).
      template {
        data        = <<EOT
{{- range nomadVar "magnetite/secrets" }}
{{ .Key }}={{ .Value }}
{{ end -}}
EOT
        destination = "secrets/magnetite.env"
        env         = true
      }

      # ── Vault alternative (enable if Vault is running) ──────────────────
      # vault {
      #   policies = ["magnetite-backend"]
      # }
      # template {
      #   data        = <<EOT
      # {{- with secret "secret/data/magnetite/backend" }}
      # DATABASE_URL={{ .Data.data.database_url }}
      # JWT_SECRET={{ .Data.data.jwt_secret }}
      # RESEND_API_KEY={{ .Data.data.resend_api_key }}
      # {{- end }}
      # EOT
      #   destination = "secrets/magnetite.env"
      #   env         = true
      # }

      env {
        SERVER_HOST          = "0.0.0.0"
        SERVER_PORT          = "8080"
        APP_ENV              = "production"
        APP_URL              = "https://api.magnetite.gg"
        FRONTEND_URL         = "https://magnetite.gg"
        CORS_ALLOWED_ORIGINS = "https://magnetite.gg"
        REDIS_URL            = "redis://{{ env "NOMAD_UPSTREAM_ADDR_redis" }}"
        # Optional: only set when THIS node hosts media.
        MEDIA_SERVER_BASE_URL = "http://{{ env "NOMAD_UPSTREAM_ADDR_mediamtx" }}"
        GAME_SERVER_WS_BASE  = "wss://runtime.magnetite.gg"
        EMAIL_PROVIDER       = "resend"
        EMAIL_FROM_ADDRESS   = "noreply@magnetite.gg"
        EMAIL_FROM_NAME      = "Magnetite"
        RUST_LOG             = "info"
        # Non-custodial settlement (§3.6); `mock` needs no external service.
        PAYMENT_RAIL         = "mock"
        PROTOCOL_FEE_BPS     = "0"
        # Pluggable comms (§3.5); `builtin` needs no external service.
        COMMS_PROVIDER       = "builtin"
        ACCESS_TOKEN_EXPIRY  = "900"
        REFRESH_TOKEN_EXPIRY = "604800"
      }

      resources {
        cpu    = 500
        memory = 512
      }

      # Service mesh upstreams (Consul Connect).
      # Enable if using Consul Connect service mesh:
      # connect {
      #   sidecar_service {
      #     proxy {
      #       upstreams {
      #         destination_name = "postgres"
      #         local_bind_port  = 5432
      #       }
      #       upstreams {
      #         destination_name = "redis"
      #         local_bind_port  = 6379
      #       }
      #       upstreams {
      #         destination_name = "mediamtx"
      #         local_bind_port  = 8888
      #       }
      #     }
      #   }
      # }

      restart {
        attempts = 5
        interval = "5m"
        delay    = "15s"
        mode     = "delay"
      }
    }
  }
}
