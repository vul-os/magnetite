#!/bin/bash
set -e

echo "Scaling Magnetite app..."

echo "Scaling to 2 instances in Johannesburg..."
fly scale count 2 --region jnb

echo "Setting memory to 512mb..."
fly scale memory 512mb

echo "Scaling complete!"
fly scale show
