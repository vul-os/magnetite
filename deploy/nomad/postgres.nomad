# Nomad job spec — PostgreSQL (single-instance, persistent volume).
#
# For production at scale, replace with a managed PostgreSQL service
# (AWS RDS, Neon, Supabase) and delete this job spec.
# Update the DATABASE_URL environment variable in backend.nomad accordingly.
#
# Requires:
#   - docker driver enabled on the Nomad agent
#   - host volume or CSI volume named "postgres_data" configured in the agent

job "postgres" {
  datacenters = ["dc1"]
  namespace   = "magnetite"
  type        = "service"

  group "postgres" {
    count = 1

    # Ensure the job runs on the same node as the volume.
    constraint {
      attribute = "${node.class}"
      value     = "data"
    }

    # Persistent volume for database files.
    volume "postgres_data" {
      type      = "host"
      read_only = false
      source    = "postgres_data"
    }

    network {
      port "db" {
        static = 5432
        to     = 5432
      }
    }

    service {
      name     = "postgres"
      port     = "db"
      provider = "consul"

      check {
        type     = "script"
        command  = "/bin/sh"
        args     = ["-c", "pg_isready -U magnetite -d magnetite"]
        interval = "10s"
        timeout  = "5s"
      }
    }

    task "postgres" {
      driver = "docker"

      config {
        image = "postgres:16-alpine"
        ports = ["db"]
      }

      volume_mount {
        volume      = "postgres_data"
        destination = "/var/lib/postgresql/data"
        read_only   = false
      }

      env {
        POSTGRES_DB       = "magnetite"
        POSTGRES_USER     = "magnetite"
        # CHANGE: set via Nomad Variables or Vault secret — never hardcode.
        POSTGRES_PASSWORD = "${POSTGRES_PASSWORD}"
      }

      resources {
        cpu    = 500   # MHz
        memory = 512   # MB
      }

      # Vault integration (recommended): fetch the password from Vault instead
      # of passing it via env.
      # vault {
      #   policies = ["magnetite-postgres"]
      # }
      # template {
      #   data        = <<EOT
      # {{- with secret "secret/data/magnetite/postgres" -}}
      # POSTGRES_PASSWORD={{ .Data.data.password }}
      # {{- end }}
      # EOT
      #   destination = "secrets/postgres.env"
      #   env         = true
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
