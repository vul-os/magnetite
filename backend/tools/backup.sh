#!/bin/bash
set -e

BACKUP_STORAGE_TYPE="${BACKUP_STORAGE_TYPE:-local}"
KEEP_DAYS="${BACKUP_KEEP_DAYS:-7}"
LOG_FILE="/var/log/magnetite_backup.log"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $1" | tee -a "$LOG_FILE" >&2
    exit 1
}

log "Starting database backup"

case "$BACKUP_STORAGE_TYPE" in
    s3)
        if [ -z "$BACKUP_S3_BUCKET" ]; then
            error "BACKUP_S3_BUCKET is not set"
        fi
        ;;
    local)
        BACKUP_LOCAL_DIR="${BACKUP_LOCAL_DIR:-/var/lib/magnetite/backups}"
        mkdir -p "$BACKUP_LOCAL_DIR"
        ;;
    *)
        error "Unknown BACKUP_STORAGE_TYPE: $BACKUP_STORAGE_TYPE"
        ;;
esac

BACKUP_ID=$(uuidgen | cut -d'-' -f1)
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
FILENAME="magnetite_backup_${TIMESTAMP}_${BACKUP_ID}.sql"
TEMP_FILE="/tmp/$FILENAME"

log "Creating SQL dump: $FILENAME"

if ! pg_dump -Fc magnetite -f "$TEMP_FILE"; then
    error "pg_dump failed"
fi

FILE_SIZE=$(stat -f%z "$TEMP_FILE" 2>/dev/null || stat -c%s "$TEMP_FILE" 2>/dev/null || echo "unknown")
log "Backup file created: $FILENAME (size: $FILE_SIZE bytes)"

case "$BACKUP_STORAGE_TYPE" in
    s3)
        log "Uploading to S3 bucket: $BACKUP_S3_BUCKET"
        if ! aws s3 cp "$TEMP_FILE" "s3://${BACKUP_S3_BUCKET}/${FILENAME}"; then
            error "Failed to upload backup to S3"
        fi
        log "Backup uploaded to S3"
        ;;
    local)
        log "Moving backup to $BACKUP_LOCAL_DIR"
        mv "$TEMP_FILE" "${BACKUP_LOCAL_DIR}/${FILENAME}"
        ;;
esac

if [ -n "$KEEP_DAYS" ] && [ "$KEEP_DAYS" -gt 0 ]; then
    log "Cleaning up backups older than $KEEP_DAYS days"

    case "$BACKUP_STORAGE_TYPE" in
        s3)
            CUTOFF_DATE=$(date -d "$KEEP_DAYS days ago" '+%Y-%m-%d')
            aws s3 ls "s3://${BACKUP_S3_BUCKET}/" | while read -r line; do
                FILE_KEY=$(echo "$line" | awk '{print $4}')
                if [ "${FILE_KEY#magnetite_backup_}" != "$FILE_KEY" ]; then
                    FILE_DATE=$(echo "$FILE_KEY" | sed 's/magnetite_backup_\([0-9]*\).*/\1/' | sed 's/\([0-9]\{4\}\)\([0-9]\{2\}\)\([0-9]\{2\}\).*/\1-\2-\3/')
                    if [[ "$FILE_DATE" < "$CUTOFF_DATE" ]]; then
                        log "Deleting old backup: $FILE_KEY"
                        aws s3 rm "s3://${BACKUP_S3_BUCKET}/${FILE_KEY}"
                    fi
                fi
            done
            ;;
        local)
            find "$BACKUP_LOCAL_DIR" -name "magnetite_backup_*.sql" -mtime "+$KEEP_DAYS" -delete 2>/dev/null || true
            ;;
    esac

    log "Cleanup completed"
fi

log "Backup completed successfully"
