#!/bin/bash
set -e

echo "Creating Fly app..."
fly apps create magnetite --no-deploy || echo "App already exists"

echo "Setting secrets..."
fly secrets set JWT_SECRET="$JWT_SECRET"
fly secrets set DATABASE_URL="$DATABASE_URL"

echo "Setting OAuth secrets..."
[ -n "$GOOGLE_CLIENT_ID" ] && fly secrets set GOOGLE_CLIENT_ID="$GOOGLE_CLIENT_ID"
[ -n "$GOOGLE_CLIENT_SECRET" ] && fly secrets set GOOGLE_CLIENT_SECRET="$GOOGLE_CLIENT_SECRET"
[ -n "$DISCORD_CLIENT_ID" ] && fly secrets set DISCORD_CLIENT_ID="$DISCORD_CLIENT_ID"
[ -n "$DISCORD_CLIENT_SECRET" ] && fly secrets set DISCORD_CLIENT_SECRET="$DISCORD_CLIENT_SECRET"
[ -n "$GITHUB_CLIENT_ID" ] && fly secrets set GITHUB_CLIENT_ID="$GITHUB_CLIENT_ID"
[ -n "$GITHUB_CLIENT_SECRET" ] && fly secrets set GITHUB_CLIENT_SECRET="$GITHUB_CLIENT_SECRET"
[ -n "$PAYSTACK_SECRET_KEY" ] && fly secrets set PAYSTACK_SECRET_KEY="$PAYSTACK_SECRET_KEY"
[ -n "$CIRCLE_API_KEY" ] && fly secrets set CIRCLE_API_KEY="$CIRCLE_API_KEY"

echo "Creating PostgreSQL database..."
fly postgres create --name magnetite-db --region jnb || echo "Postgres may already exist"
fly postgres attach --app magnetite magnetite-db || echo "Postgres already attached"

echo "Creating volumes..."
fly volumes create pg_data --region jnb --size 10 || echo "Volume may already exist"

echo "Deploying..."
fly deploy

echo "Setup complete!"
echo "Run 'fly scale count 2 --region jnb' to scale to 2 instances"
