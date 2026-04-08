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

demo: ## Build the demo site (render AsciiDoc + zensical build)
	cargo run --example render_asciidoc
	cd demo && zensical build --clean

serve: ## Render AsciiDoc and serve with zensical
	cargo run --example render_asciidoc
	cd demo && zensical serve

clean: ## Clean build artifacts and demo site
	cargo clean
	rm -rf demo/site
	rm -f demo/docs/asciidoc.md demo/docs/features.md
