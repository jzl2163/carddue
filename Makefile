.PHONY: init dev-services api frontend-install dev-backend dev-frontend test build up
init:
	node scripts/init-env.mjs --development
dev-services:
	docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d postgres mailpit
api:
	cargo run --bin carddue -- openapi > frontend/openapi.json
	cd frontend && npm run api:generate
frontend-install:
	cd frontend && npm ci
	$(MAKE) api
dev-backend:
	node scripts/dev-backend.mjs
dev-frontend:
	cd frontend && npm run dev
test:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace --all-targets
	cd frontend && npm run check && npm test && npm run build && npm run budget
build:
	docker compose build
up:
	docker compose up -d
