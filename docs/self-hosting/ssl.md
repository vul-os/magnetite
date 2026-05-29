# SSL/TLS Guide

Secure your Magnetite deployment with HTTPS.

## Using Let's Encrypt

### Automatic Certificate Generation

Using Certbot with Nginx:

```bash
# Install Certbot
sudo apt update
sudo apt install certbot python3-certbot-nginx

# Generate certificate
sudo certbot --nginx -d magnetite.example.com

# Auto-renew (already set up by installer)
sudo certbot renew --dry-run
```

### Docker with Let's Encrypt

Use the `certbot` container:

```yaml
services:
  certbot:
    image: certbot/certbot:latest
    volumes:
      - ./certbot/conf:/etc/letsencrypt
      - ./certbot/www:/var/www/certbot
    entrypoint: "/bin/sh -c 'trap exit TERM; while :; do certbot renew; sleep 12h; done;'"

  nginx:
    image: nginx:alpine
    volumes:
      - ./certbot/conf:/etc/letsencrypt
      - ./certbot/www:/var/www/certbot
      - ./nginx.conf:/etc/nginx/conf.d/default.conf
    ports:
      - "80:80"
      - "443:443"
```

## Nginx Configuration

### Production Nginx Config

```nginx
upstream backend {
    server backend:8080;
    keepalive 32;
}

server {
    listen 80;
    server_name magnetite.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name magnetite.example.com;

    # SSL Certificate
    ssl_certificate /etc/letsencrypt/live/magnetite.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/magnetite.example.com/privkey.pem;

    # SSL Settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;

    # Security Headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' https: wss:; font-src 'self' data:;" always;

    # HSTS (uncomment after confirming HTTPS works)
    # add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;

    root /usr/share/nginx/html;
    index index.html;

    # Gzip compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_proxied any;
    gzip_types text/plain text/css text/xml text/javascript application/javascript application/json application/xml;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=login_limit:10m rate=1r/s;

    client_max_body_size 10M;

    location / {
        try_files $uri $uri/ /index.html;
        expires 1h;
        add_header Cache-Control "public, immutable";
    }

    location /api {
        limit_req zone=api_limit burst=20 nodelay;

        proxy_pass http://backend;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_cache_bypass $http_upgrade;

        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    location /health {
        access_log off;
        return 200 "healthy\n";
        add_header Content-Type text/plain;
    }

    # Let's Encrypt ACME challenge
    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }
}
```

### Renewal Hook for Nginx

Create `/etc/letsencrypt/renewal-hooks/post/magnetite.conf`:

```bash
#!/bin/bash
nginx -s reload
```

Make executable:

```bash
chmod +x /etc/letsencrypt/renewal-hooks/post/magnetite.conf
```

## Certificate Renewal

### Manual Renewal

```bash
# Test renewal (dry run)
sudo certbot renew --dry-run

# Force renewal
sudo certbot renew --force-renewal

# For specific domain
sudo certbot renew --cert-name magnetite.example.com
```

### Automatic Renewal

Certbot installs a timer by default. Verify:

```bash
# Check timer status
sudo systemctl status certbot.timer

# List timers
sudo systemctl list-timers
```

Add to crontab if needed:

```bash
sudo crontab -e

# Add: check twice daily
0 0,12 * * * certbot renew --post-hook "nginx -s reload"
```

## HSTS Setup

HTTP Strict Transport Security forces browsers to use HTTPS.

### Immediate Enablement

Add header to Nginx:

```nginx
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
```

### Submission for Browser Preloading

After running with HSTS for 1 year, submit to:
https://hstspreload.org/

### Subdomains

If you have subdomains:

```nginx
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
```

**Warning**: Once enabled, you cannot easily disable HTTPS. Ensure SSL is working before enabling.

## SSL Certificate Monitoring

### Check Expiration

```bash
# Using openssl
openssl s_client -connect magnetite.example.com:443 -servername magnetite.example.com 2>/dev/null | openssl x509 -noout -dates

# Using certbot
sudo certbot certificates
```

### Expiration Warning Script

Create `check_cert.sh`:

```bash
#!/bin/bash

DOMAIN="magnetite.example.com"
EMAIL="admin@example.com"
DAYS=30

EXPIRY=$(openssl s_client -connect "$DOMAIN:443" -servername "$DOMAIN" 2>/dev/null | openssl x509 -noout -enddate | cut -d= -f2)
EXPIRY_SECS=$(date -d "$EXPIRY" +%s)
NOW_SECS=$(date +%s)
DAYS_LEFT=$(( ($EXPIRY_SECS - $NOW_SECS) / 86400 ))

if [ $DAYS_LEFT -le $DAYS ]; then
    echo "Certificate for $DOMAIN expires in $DAYS_LEFT days" | mail -s "SSL Certificate Expiring" "$EMAIL"
fi
```

## Self-Signed Certificates (Development Only)

### Generate Self-Signed Cert

```bash
# Create directory
mkdir -p certs

# Generate certificate
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout certs/server.key \
  -out certs/server.crt \
  -subj "/C=US/ST=State/L=City/O=Magnetite/CN=localhost"
```

**Warning**: Never use self-signed certificates in production.

### Docker Development Config

```yaml
services:
  frontend:
    # ...
    volumes:
      - ./certs/server.crt:/etc/nginx/certs/server.crt:ro
      - ./certs/server.key:/etc/nginx/certs/server.key:ro
    environment:
      - NGINX_SSL_CERT=/etc/nginx/certs/server.crt
      - NGINX_SSL_KEY=/etc/nginx/certs/server.key
```

## Certificate Chains

### Check Certificate Chain

```bash
openssl s_client -connect magnetite.example.com:443 -showcerts
```

### Fix Missing Intermediate Certificates

Download and include intermediate certificates:

```bash
# Get certificate chain from provider
# For Let's Encrypt:
curl -s https://letsencrypt.org/certs/lets-encrypt-r3.pem >> certs/fullchain.pem
```

## Security Best Practices

1. **Use TLS 1.3 only if possible**
   ```nginx
   ssl_protocols TLSv1.3;
   ```

2. **Disable older protocols**
   ```nginx
   ssl_protocols TLSv1.2 TLSv1.3;
   ```

3. **Enable OCSP stapling**
   ```nginx
   ssl_stapling on;
   ssl_stapling_verify on;
   resolver 8.8.8.8 8.8.4.4 valid=300s;
   ```

4. **Use strong cipher suites**
   ```nginx
   ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
   ```

5. **Redirect all HTTP to HTTPS**
   ```nginx
   server {
       listen 80;
       return 301 https://$server_name$request_uri;
   }
   ```

## Troubleshooting

### Certificate Mismatch

Check that the certificate CN matches your domain:

```bash
openssl x509 -in certs/server.crt -noout -subject
```

### Mixed Content Errors

Ensure all resources load over HTTPS. Check browser console for warnings.

### OCSP Stapling Errors

```nginx
ssl_trusted_certificate /etc/letsencrypt/live/magnetite.example.com/chain.pem;
```
