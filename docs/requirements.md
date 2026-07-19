# System Requirements

> **There is no capacity tier to buy.** A `magnetite` node measures its own
> cores, RAM, and bandwidth on start and advertises what it can hold. Shard
> count and player cap are **emergent from the hardware**, not config
> constants — the numbers below are floors for running the software, not
> quotas. See [Hosting a server](./hosting-a-server.md).
>
> The Postgres/Redis requirements apply to the full platform backend
> (storefront, accounts, comms). `magnetite dev` needs neither.

## Server Requirements

### Minimum
- 1 vCPU
- 512MB RAM
- 10GB SSD storage
- Ubuntu 20.04 or similar Linux

### Recommended
- 2 vCPUs
- 2GB RAM
- 50GB SSD storage
- Ubuntu 22.04 LTS

### For Production
- 4+ vCPUs
- 8GB+ RAM
- 100GB+ SSD
- PostgreSQL 15+
- Redis 7+ (optional, for caching)

## Client Requirements

### Desktop
- Chrome 90+, Firefox 88+, Safari 14+, Edge 90+
- WebSocket support required
- JavaScript enabled

### Mobile
- iOS 14+ Safari
- Android 10+ Chrome

## Network
- Ports 80, 443 open (HTTP/HTTPS)
- Port 8080 for backend API
- Outbound WebSocket support
