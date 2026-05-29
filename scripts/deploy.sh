#!/bin/bash
set -e
fly deploy
fly scale count 1
fly secrets set DATABASE_URL=$DATABASE_URL
fly secrets set JWT_SECRET=$JWT_SECRET
