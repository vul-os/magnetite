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

## Payment Problems

Payments are **non-custodial**: the buyer pays the seller wallet-to-wallet
through the `PaymentRail` seam and the node holds no funds. There is no payment
provider account, no API keys, no sandbox/live toggle, no payment webhook, no
card, and no deposit/withdrawal/payout to debug. What can actually go wrong is
**receipt verification**.

See [Self-hosting → Troubleshooting](./self-hosting/troubleshooting.md#payment-issues)
for the full runbook. In short:

### Purchase or paid tier rejected despite payment

1. Check the rail: `PAYMENT_RAIL=mock` (default, offline, deterministic
   signatures) and `PROTOCOL_FEE_BPS=0` (default). `CHAIN_RPC_URL`, `CHAIN_ID`,
   and `STABLECOIN_ADDRESS` are **unused placeholders** — no real chain rail is
   implemented, so setting them changes nothing.
2. The signature is re-verified on every access; a `payment_receipts` row alone
   is not sufficient. Grep the logs for receipt verification failures.
3. Selling hosting or paid tiers requires `OPERATOR_WALLET_PUBKEY`; unset, paid
   tiers fail closed.

### "Wallet not linked"

Checkout needs a payee. Both buyer and developer need an address linked via
`POST /api/v1/wallet/link`.

### Access denied on a paid room or paid session

Paid access requires a **proven** (linked) key, not a derived one. Legacy
accounts that never linked a wallet keep working on free and builtin paths but
are refused on paid ones — by design, so a back-filled row cannot buy access.
