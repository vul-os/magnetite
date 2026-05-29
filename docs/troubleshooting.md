# Troubleshooting

## Common Deployment Issues

### Application Won't Start

**Symptom:** Application fails to start or exits immediately after launch.

**Solutions:**
- Verify all environment variables are set correctly in `.env`
- Check logs in `logs/app.log` for specific error messages
- Ensure the database is running and accessible
- Confirm port 8080 is not in use by another process: `lsof -i :8080`

### Container Crashes

**Symptom:** Docker container exits with non-zero status code.

**Solutions:**
- Check container logs: `docker logs <container_name>`
- Verify volume mounts are correct and have proper permissions
- Ensure base image is compatible with your architecture

### SSL/TLS Certificate Errors

**Symptom:** Browser shows certificate warnings or connection refused errors.

**Solutions:**
- Verify certificate files exist at configured paths
- Check certificate expiration dates
- For Let's Encrypt, ensure port 80 is open for validation
- Restart the web server after certificate renewal

## Database Connection Problems

### Connection Refused

**Symptom:** `ECONNREFUSED` error when connecting to PostgreSQL.

**Solutions:**
1. Verify PostgreSQL is running: `systemctl status postgresql`
2. Check the connection string in `.env` uses correct host/port
3. Ensure `pg_hba.conf` allows connections from your application server
4. Test connection manually: `psql -h <host> -p <port> -U <user> <database>`

### Authentication Failed

**Symptom:** `FATAL: password authentication failed` error.

**Solutions:**
1. Verify username and password in connection string
2. Check PostgreSQL user has correct privileges: `GRANT ALL ON DATABASE <db> TO <user>;`
3. For local development, ensure `pg_hba.conf` uses `trust` or `md5` method appropriately

### Migration Failures

**Symptom:** Database migrations fail to apply.

**Solutions:**
1. Ensure you have write access to the database
2. Check for conflicting existing schema changes
3. Review failed migration logs for specific SQL errors
4. Backup data before running destructive migrations

## OAuth Callback Issues

### Redirect URI Mismatch

**Symptom:** Provider returns "redirect_uri_mismatch" error.

**Solutions:**
1. Verify callback URL in application matches exactly what's registered with the OAuth provider
2. Check for trailing slashes or missing protocol (http vs https)
3. For local development, ensure redirect URI includes port if non-standard
4. Update OAuth provider settings if callback URL has changed

### Invalid State Parameter

**Symptom:** Authentication fails with state validation error.

**Solutions:**
1. Ensure cookies are enabled and not blocked
2. Check for clock skew between servers (NTP sync can help)
3. Clear browser cookies and try again
4. Verify session middleware is configured correctly

### Token Exchange Failures

**Symptom:** Can authenticate but cannot retrieve access token.

**Solutions:**
1. Verify client secret is correct
2. Check that required scopes are requested
3. Ensure token endpoint URL is correct
4. Check provider rate limits or usage quotas

## Payment Processing Problems

### Webhook Delivery Failures

**Symptom:** Payment confirmed but order not updated.

**Solutions:**
1. Verify webhook endpoint is publicly accessible
2. Check webhook secret matches provider configuration
3. Review webhook logs for delivery attempts and failures
4. Use provider dashboard to replay missed webhooks
5. Implement idempotency to handle duplicate deliveries

### Sandbox vs Production Confusion

**Symptom:** Test payments work but live payments fail.

**Solutions:**
1. Verify you are using correct API keys for the environment
2. Check that live mode is enabled in your payment provider dashboard
3. Ensure webhook endpoints match the current environment
4. Verify PCI compliance requirements are met for live mode

### Currency/Amount Mismatches

**Symptom:** Payment amount doesn't match order total.

**Solutions:**
1. Verify amount is passed in smallest currency unit (cents, pence)
2. Check currency code matches between application and payment provider
3. Ensure amount calculation includes all fees and discounts
4. Use provider's amount validation tools before creating charges

### Card Declines

**Symptom:** Customer reports card was declined.

**Solutions:**
1. Check decline reason code from payment provider
2. Common causes: insufficient funds, card expired, fraud detection
3. Ask customer to contact their bank for more details
4. Suggest alternative payment methods if available
