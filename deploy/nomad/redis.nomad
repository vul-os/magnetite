# Nomad job spec — Redis (single-instance, AOF persistence).
#
# For production at scale, replace with a managed Redis service
# (AWS ElastiCache, Upstash) and update REDIS_URL in backend.nomad.

job "redis" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  group "redis" {
    count = 1

    constraint {
      attribute = "${node.class}"
      value     = "data"
    }

    volume "redis_data" {
      type      = "host"
      read_only = false
      source    = "redis_data"
    }

    network {
      port "redis" {
        static = 6379
        to     = 6379
      }
    }

    service {
      name     = "redis"
      port     = "redis"
      provider = "consul"

      check {
        type     = "script"
        command  = "/bin/sh"
        args     = ["-c", "redis-cli ping"]
        interval = "10s"
        timeout  = "5s"
      }
    }

    task "redis" {
      driver = "docker"

      config {
        image   = "redis:7-alpine"
        ports   = ["redis"]
        command = "redis-server"
        args    = ["--appendonly", "yes"]
      }

      volume_mount {
        volume      = "redis_data"
        destination = "/data"
        read_only   = false
      }

      resources {
        cpu    = 200
        memory = 256
      }

      restart {
        attempts = 5
        interval = "5m"
        delay    = "10s"
        mode     = "delay"
      }
    }
  }
}
