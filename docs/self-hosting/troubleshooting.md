# Troubleshooting

Solutions for common issues when self-hosting Magnetite.

## Subscription Issues

### Payments not processing

**Symptoms**: Users cannot subscribe, payments fail silently.

**Solutions**:

1. Verify payment provider credentials are set:
   ```bash
   # At least one provider must be configured
   PAYSTACK_SECRET_KEY=your_paystack_key
   CIRCLE_API_KEY=your_circle_key
   SUBSCRIPTION_WEBHOOK_SECRET=your_webhook_secret
   ```

2. Check webhook configuration:
   - Ensure webhook endpoints are reachable from your payment provider
   - Verify `SUBSCRIPTION_WEBHOOK_SECRET` matches your provider's webhook secret
   - Webhook URLs typically follow: `https://your-domain.com/api/webhooks/subscription`

3. Review payment provider dashboard for:
   - API key status (active/suspended)
   - Webhook delivery failures
   - Transaction logs for specific error codes

### Webhook not receiving events

**Symptoms**: Subscription status doesn't update after payment.

**Solutions**:

1. Verify webhook secret matches your payment provider configuration
2. Check logs for incoming webhook requests:
   ```bash
   docker compose logs api | grep webhook
   ```
3. Ensure your deployment is accessible from the internet on port 443
4. Check your payment provider's webhook delivery logs for failure reasons

### Subscription shows as "past_due"

**Symptoms**: User paid but subscription still shows past due status.

**Solutions**:

1. Webhook may not have been received - check webhook logs
2. Manually verify payment in payment provider dashboard
3. Update subscription status manually via admin interface if needed

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
