# Troubleshooting

Solutions for common issues when self-hosting Magnetite.

## Payment Issues

Payments are non-custodial: the buyer pays the seller wallet-to-wallet through the
`PaymentRail` seam and the node holds no funds. There is no payment provider
account, no deposit, no withdrawal and no payout to debug. What can go wrong is
receipt verification.

### Purchases or subscriptions rejected

**Symptoms**: A purchase or paid tier is refused even though payment was made.

**Solutions**:

1. Check the configured rail:
   ```bash
   PAYMENT_RAIL=mock          # default — deterministic signed receipts, offline
   PROTOCOL_FEE_BPS=0         # default — fee rides on top of the subtotal
   ```
   `mock` needs no external service. `CHAIN_RPC_URL`, `CHAIN_ID` and
   `STABLECOIN_ADDRESS` are unused placeholders — no real chain rail is
   implemented yet, so setting them changes nothing.

2. The node re-verifies the rail signature on every access, so a receipt row in
   the database is not sufficient. Check the logs for verification failures:
   ```bash
   docker compose logs api | grep -i receipt
   ```

3. If this node sells hosting or paid tiers, confirm `OPERATOR_WALLET_PUBKEY` is
   set to the hex Ed25519 pubkey that should receive the operator split.

### Entitlement missing after a purchase

**Symptoms**: The buyer paid but does not have access.

**Solutions**:

1. The signed receipt *is* the entitlement. Confirm the receipt exists and
   verifies — a database row alone never grants access.
2. Confirm the receipt has not been voided. A refund voids the receipt and
   revokes the entitlement; no balance is moved, so there is nothing to reconcile
   on a ledger.
3. Confirm the buyer's linked wallet address matches the receipt's buyer pubkey.

## Database Issues

### Connection refused

**Symptoms**: `could not connect to server: Connection refused`

**Solutions**:

- Verify PostgreSQL is running: `docker compose ps db`
- Check `DATABASE_URL` format is correct
- Ensure PostgreSQL port (5432) is accessible

### Migration failures

**Symptoms**: App fails to start with migration errors.

**Solutions**:

```bash
# Run migrations manually
docker compose exec api /app/migrations/run

# Check migration status
docker compose exec api /app/migrations/status
```

## Authentication Issues

### JWT validation fails

**Symptoms**: Users get logged out immediately, tokens rejected.

**Solutions**:

- Verify `JWT_SECRET` is consistent across restarts
- Ensure `JWT_SECRET` is at least 32 characters
- Check for whitespace or encoding issues in `.env` file

## Performance Issues

### High memory usage

**Solutions**:

- Reduce game-host replicas in docker-compose.yml
- Limit container resources in deployment config
- Enable Redis connection pooling if not already

### Slow response times

**Solutions**:

- Check database connection pooling settings
- Verify Redis is being used for caching
- Review slow query logs in PostgreSQL
