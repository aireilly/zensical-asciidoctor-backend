.PHONY: build test test-integration check fmt clippy clean demo serve help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

build: ## Build the library
	cargo build

test: ## Run unit tests
	cargo test

test-integration: ## Run integration tests (requires asciidoctor)
	cargo test --features integration

test-all: test test-integration ## Run all tests

check: fmt clippy test ## Run fmt, clippy, and tests

fmt: ## Check formatting
	cargo fmt --check

clippy: ## Run clippy lints
	cargo clippy --all-targets --all-features -- -D warnings

demo: ## Build the demo site (zensical build + render AsciiDoc pages)
	cd demo && zensical build --clean
	cargo run --example render_asciidoc

serve: demo ## Build and serve the demo site at http://localhost:8123
	@echo "Serving demo site at http://localhost:8123 (Ctrl+C to stop)"
	python3 -m http.server 8123 -d demo/site

clean: ## Clean build artifacts and demo site
	cargo clean
	rm -rf demo/site
