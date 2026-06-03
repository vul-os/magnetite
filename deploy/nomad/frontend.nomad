# Nomad job spec — Magnetite frontend (nginx SPA + reverse proxy).
# Image: ghcr.io/magnetite/frontend:latest
# Port: 80
# Health: GET /health → 200 "OK"

job "frontend" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  update {
    max_parallel      = 1
    min_healthy_time  = "15s"
    healthy_deadline  = "3m"
    progress_deadline = "10m"
    auto_revert       = true
  }

  group "frontend" {
    count = 2

    spread {
      attribute = "${node.unique.id}"
    }

    network {
      port "http" {
        to = 80
      }
    }

    service {
      name     = "frontend"
      port     = "http"
      provider = "consul"

      check {
        type     = "http"
        path     = "/health"
        interval = "15s"
        timeout  = "5s"
      }
    }

    task "frontend" {
      driver = "docker"

      config {
        image = "ghcr.io/magnetite/frontend:latest"
        ports = ["http"]
      }

      resources {
        cpu    = 100
        memory = 128
      }

      restart {
        attempts = 3
        interval = "5m"
        delay    = "10s"
        mode     = "delay"
      }
    }
  }
}
