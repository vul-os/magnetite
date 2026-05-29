.PHONY: help dev build test lint clean docker-up docker-down migrate

# Backend commands
backend-build:
	cd backend && cargo build

backend-run:
	cd backend && cargo run

backend-test:
	cd backend && cargo test

backend-lint:
	cd backend && cargo fmt && cargo clippy

# Frontend commands
frontend-install:
	npm install

frontend-dev:
	npm run dev

frontend-build:
	npm run build

frontend-test:
	npm test

frontend-lint:
	npm run lint

# Docker commands
docker-up:
	docker-compose up -d

docker-down:
	docker-compose down

# Database
migrate:
	cd backend && ./tools/migrate.sh up

migrate-reset:
	cd backend && ./tools/migrate.sh reset

# Full stack
dev: docker-up migrate
	cd frontend && npm run dev

build: backend-build frontend-build

test: backend-test frontend-test

# Development
setup: frontend-install backend-build

clean:
	cd backend && cargo clean
	rm -rf frontend/dist