#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

usage() {
    echo -e "${BOLD}Usage:${NC} $0 <command> [options]"
    echo ""
    echo -e "${BOLD}Commands:${NC}"
    echo "  up <migration_name>     Run pending migrations up to the specified one"
    echo "  down <migration_name>  Rollback migrations down to the specified one"
    echo "  status                 Show migration status"
    echo "  reset                  Reset database (rollback all migrations)"
    echo "  info <migration_name>  Show information about a migration"
    echo ""
    echo -e "${BOLD}Options:${NC}"
    echo "  --dry-run              Show what would be done without executing"
    echo "  -h, --help            Show this help message"
    echo ""
    echo -e "${BOLD}Environment:${NC}"
    echo "  DATABASE_URL           PostgreSQL connection string"
    echo "                        (default: postgres://postgres:postgres@localhost:5432/magnetite)"
    exit 1
}

error() {
    echo -e "${RED}ERROR:${NC} $1" >&2
    exit 1
}

info() {
    echo -e "${BLUE}INFO:${NC} $1"
}

success() {
    echo -e "${GREEN}SUCCESS:${NC} $1"
}

warn() {
    echo -e "${YELLOW}WARNING:${NC} $1"
}

check_postgres() {
    if ! command -v psql &> /dev/null; then
        error "psql is not installed. PostgreSQL client is required for this operation."
    fi
}

parse_version() {
    local migration=$1
    echo "$migration" | grep -oE '^[0-9]{14}' | head -1
}

get_migrations() {
    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    if [ ! -d "$migrations_dir" ]; then
        error "Migrations directory not found: $migrations_dir"
    fi
    ls -1 "$migrations_dir"/*.sql 2>/dev/null | xargs -I{} basename {} | sort
}

run_sql() {
    local sql=$1
    local description=$2
    local dry_run=${DRY_RUN:-false}

    if [ "$dry_run" = "true" ]; then
        echo -e "${CYAN}[DRY-RUN]${NC} Would execute:"
        echo "$sql" | sed 's/^/    /'
    else
        info "$description"
        echo "$sql" | psql "$DATABASE_URL" -q -t 2>/dev/null || error "Failed to execute SQL"
    fi
}

cmd_status() {
    check_postgres
    info "Checking migration status..."

    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    local db_version=""

    db_version=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1;" 2>/dev/null | tr -d ' ' || echo "")

    echo ""
    echo -e "${BOLD}Database Schema Version:${NC} ${CYAN}${db_version:-None}${NC}"
    echo ""
    echo -e "${BOLD}Available Migrations:${NC}"
    echo ""

    local applied=""
    applied=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations ORDER BY version;" 2>/dev/null | tr -d ' ' || echo "")

    get_migrations | while read -r migration; do
        local version=$(parse_version "$migration")
        local name=$(echo "$migration" | sed 's/^[0-9]*_//' | sed 's/.sql$//')

        if echo "$applied" | grep -q "^${version}$"; then
            echo -e "  ${GREEN}✓${NC} $version - $name (applied)"
        else
            echo -e "  ${YELLOW}○${NC} $version - $name (pending)"
        fi
    done

    local pending_count=$(get_migrations | wc -l | tr -d ' ')
    local applied_count=$(echo "$applied" | grep -c "." 2>/dev/null || echo "0")
    pending_count=$((pending_count - applied_count))

    echo ""
    echo -e "${BOLD}Summary:${NC} $applied_count applied, $pending_count pending"
}

cmd_info() {
    local migration_name=$1

    if [ -z "$migration_name" ]; then
        error "Migration name is required for info command"
    fi

    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    local migration_file=""

    migration_file=$(ls "$migrations_dir"/*${migration_name}*.sql 2>/dev/null | head -1)

    if [ -z "$migration_file" ]; then
        migration_file=$(ls "$migrations_dir"/${migration_name}.sql 2>/dev/null || echo "")
    fi

    if [ -z "$migration_file" ]; then
        error "Migration not found: $migration_name"
    fi

    local version=$(parse_version "$(basename "$migration_file")")
    local name=$(echo "$(basename "$migration_file")" | sed 's/^[0-9]*_//' | sed 's/.sql$//')
    local line_count=$(wc -l < "$migration_file")
    local file_size=$(ls -lh "$migration_file" | awk '{print $5}')

    echo ""
    echo -e "${BOLD}Migration Information${NC}"
    echo -e "${BOLD}======================${NC}"
    echo ""
    echo -e "${BOLD}Name:${NC}    $name"
    echo -e "${BOLD}Version:${NC} $version"
    echo -e "${BOLD}File:${NC}    $(basename "$migration_file")"
    echo -e "${BOLD}Size:${NC}    $file_size ($line_count lines)"
    echo ""

    check_postgres
    local applied=""
    applied=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations WHERE version = '$version';" 2>/dev/null | tr -d ' ' || echo "")

    if [ -n "$applied" ]; then
        local applied_at=""
        applied_at=$(psql "$DATABASE_URL" -t -c "SELECT applied_at FROM schema_migrations WHERE version = '$version';" 2>/dev/null | tr -d ' ' || echo "")
        echo -e "Status: ${GREEN}Applied${NC} at $applied_at"
    else
        echo -e "Status: ${YELLOW}Pending${NC}"
    fi
    echo ""
}

cmd_up() {
    local target_migration=${1:-}
    local dry_run=${DRY_RUN:-false}

    check_postgres

    info "Running migrations..."

    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    local applied=""
    applied=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations ORDER BY version;" 2>/dev/null | tr -d ' ' || echo "")

    local migrations_to_run=""

    if [ -z "$target_migration" ]; then
        migrations_to_run=$(get_migrations | while read -r migration; do
            local version=$(parse_version "$migration")
            if ! echo "$applied" | grep -q "^${version}$"; then
                echo "$migration"
            fi
        done)
    else
        local target_file=""
        target_file=$(ls "$migrations_dir"/*${target_migration}*.sql 2>/dev/null | head -1)

        if [ -z "$target_file" ]; then
            target_file=$(ls "$migrations_dir"/${target_migration}.sql 2>/dev/null || echo "")
        fi

        if [ -z "$target_file" ]; then
            error "Migration not found: $target_migration"
        fi

        local target_version=$(parse_version "$(basename "$target_file")")
        local reached_target=false

        migrations_to_run=$(get_migrations | while read -r migration; do
            local version=$(parse_version "$migration")
            if [ "$reached_target" = "true" ]; then
                continue
            fi
            if ! echo "$applied" | grep -q "^${version}$"; then
                if [ "$version" = "$target_version" ]; then
                    reached_target=true
                fi
                echo "$migration"
            fi
        done)
    fi

    if [ -z "$migrations_to_run" ]; then
        success "No migrations to run"
        return 0
    fi

    local count=$(echo "$migrations_to_run" | grep -c "." 2>/dev/null || echo "0")
    info "Found $count migration(s) to run"

    if [ "$dry_run" = "true" ]; then
        echo -e "${CYAN}[DRY-RUN]${NC} The following migrations would be run:"
        echo "$migrations_to_run" | while read -r m; do
            echo -e "  ${GREEN}→${NC} $m"
        done
        return 0
    fi

    echo "$migrations_to_run" | while read -r migration; do
        local version=$(parse_version "$migration")
        local name=$(echo "$migration" | sed 's/^[0-9]*_//' | sed 's/.sql$//')

        info "Running: $version - $name"

        local sql_content
        sql_content=$(cat "${migrations_dir}/${migration}")

        echo "$sql_content" | psql "$DATABASE_URL" -q 2>/dev/null || error "Failed to run migration: $migration"

        psql "$DATABASE_URL" -t -c "INSERT INTO schema_migrations (version, name) VALUES ('$version', '$name');" >/dev/null 2>&1 || true

        success "Applied: $version - $name"
    done

    success "All migrations completed successfully"
}

cmd_down() {
    local target_migration=${1:-}

    if [ -z "$target_migration" ]; then
        error "Target migration name is required for down command"
    fi

    check_postgres
    local dry_run=${DRY_RUN:-false}

    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    local target_file=""
    target_file=$(ls "$migrations_dir"/*${target_migration}*.sql 2>/dev/null | head -1)

    if [ -z "$target_file" ]; then
        target_file=$(ls "$migrations_dir"/${target_migration}.sql 2>/dev/null || echo "")
    fi

    if [ -z "$target_file" ]; then
        error "Migration not found: $target_migration"
    fi

    local target_version=$(parse_version "$(basename "$target_file")")

    info "Rolling back to: $target_version"

    local applied=""
    applied=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations WHERE version = '$target_version' ORDER BY version DESC;" 2>/dev/null | tr -d ' ' || echo "")

    if [ -z "$applied" ]; then
        error "Migration $target_version is not applied"
    fi

    local migrations_to_rollback=""
    migrations_to_rollback=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations WHERE version > '$target_version' ORDER BY version DESC;" 2>/dev/null | tr -d ' ' || echo "")

    if [ -z "$migrations_to_rollback" ]; then
        success "No migrations to rollback"
        return 0
    fi

    local count=$(echo "$migrations_to_rollback" | grep -c "." 2>/dev/null || echo "0")
    info "Found $count migration(s) to rollback"

    if [ "$dry_run" = "true" ]; then
        echo -e "${CYAN}[DRY-RUN]${NC} The following migrations would be rolled back:"
        echo "$migrations_to_rollback" | while read -r v; do
            local name=$(ls "$migrations_dir"/*${v}*.sql 2>/dev/null | xargs -I{} basename {} | sed 's/^[0-9]*_//' | sed 's/.sql$//')
            echo -e "  ${YELLOW}←${NC} $v - $name"
        done
        return 0
    fi

    echo "$migrations_to_rollback" | while read -r version; do
        local migration_file=""
        migration_file=$(ls "$migrations_dir"/*${version}*.sql 2>/dev/null | head -1)
        local name=$(echo "$(basename "$migration_file")" | sed 's/^[0-9]*_//' | sed 's/.sql$//')

        info "Rolling back: $version - $name"

        psql "$DATABASE_URL" -t -c "DELETE FROM schema_migrations WHERE version = '$version';" >/dev/null 2>&1 || true

        success "Rolled back: $version - $name"
    done

    success "Rollback completed successfully"
}

cmd_reset() {
    check_postgres
    local dry_run=${DRY_RUN:-false}

    warn "This will rollback ALL migrations!"

    if [ "$dry_run" = "true" ]; then
        echo -e "${CYAN}[DRY-RUN]${NC} All migrations would be rolled back"
        return 0
    fi

    read -p "Are you sure? Type 'yes' to confirm: " confirm
    if [ "$confirm" != "yes" ]; then
        info "Cancelled"
        return 0
    fi

    local migrations_dir="${MIGRATIONS_DIR:-./migrations}"
    local applied=""
    applied=$(psql "$DATABASE_URL" -t -c "SELECT version FROM schema_migrations ORDER BY version DESC;" 2>/dev/null | tr -d ' ' || echo "")

    if [ -z "$applied" ]; then
        success "No migrations to reset"
        return 0
    fi

    echo "$applied" | while read -r version; do
        local migration_file=""
        migration_file=$(ls "$migrations_dir"/*${version}*.sql 2>/dev/null | head -1)
        local name=$(echo "$(basename "$migration_file")" | sed 's/^[0-9]*_//' | sed 's/.sql$//')

        info "Rolling back: $version - $name"

        psql "$DATABASE_URL" -t -c "DELETE FROM schema_migrations WHERE version = '$version';" >/dev/null 2>&1 || true

        success "Rolled back: $version - $name"
    done

    success "Reset completed successfully"
}

cmd_version_check() {
    check_postgres

    info "Checking PostgreSQL connection..."

    local version
    version=$(psql "$DATABASE_URL" -t -c "SELECT version();" 2>/dev/null | head -1 | tr -d ' ')

    if [ -z "$version" ]; then
        error "Could not connect to PostgreSQL"
    fi

    local major_version
    major_version=$(echo "$version" | grep -oE 'PostgreSQL [0-9]+' | awk '{print $2}')

    echo ""
    echo -e "${BOLD}PostgreSQL Version Check${NC}"
    echo -e "${BOLD}=======================${NC}"
    echo ""
    echo -e "${BOLD}Server Version:${NC} $version"
    echo ""

    if [ "$major_version" -ge 14 ]; then
        echo -e "${GREEN}✓${NC} PostgreSQL $major_version is supported (requires 14+)"
    else
        echo -e "${YELLOW}!${NC} PostgreSQL $major_version may work, but 14+ is recommended"
    fi

    local db_name
    db_name=$(echo "$DATABASE_URL" | grep -oE '/[^/]+$' | tr -d '/')
    local db_exists
    db_exists=$(psql "$DATABASE_URL" -t -c "SELECT 1 FROM pg_database WHERE datname = '$db_name';" 2>/dev/null | tr -d ' ' || echo "")

    echo ""
    if [ -n "$db_exists" ]; then
        echo -e "${GREEN}✓${NC} Database '$db_name' exists"
    else
        echo -e "${YELLOW}!${NC} Database '$db_name' does not exist"
    fi

    local table_exists
    table_exists=$(psql "$DATABASE_URL" -t -c "SELECT 1 FROM information_schema.tables WHERE table_name = 'schema_migrations';" 2>/dev/null | tr -d ' ' || echo "")

    echo ""
    if [ -n "$table_exists" ]; then
        echo -e "${GREEN}✓${NC} schema_migrations table exists"
    else
        echo -e "${YELLOW}!${NC} schema_migrations table does not exist (run migrations first)"
    fi
}

if [ -z "$DATABASE_URL" ]; then
    info "DATABASE_URL not set, using default"
    export DATABASE_URL="postgres://postgres:postgres@localhost:5432/magnetite"
fi

export MIGRATIONS_DIR="${MIGRATIONS_DIR:-./migrations}"

DRY_RUN=false
COMMAND=""
ARG1=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        up|down|status|reset|info|version-check)
            COMMAND="$1"
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        -*)
            error "Unknown option: $1"
            ;;
        *)
            if [ -z "$ARG1" ]; then
                ARG1="$1"
            fi
            shift
            ;;
    esac
done

case "$COMMAND" in
    up)
        cmd_up "$ARG1"
        ;;
    down)
        cmd_down "$ARG1"
        ;;
    status)
        cmd_status
        ;;
    reset)
        cmd_reset
        ;;
    info)
        cmd_info "$ARG1"
        ;;
    version-check)
        cmd_version_check
        ;;
    *)
        usage
        ;;
esac