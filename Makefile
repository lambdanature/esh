# Variables
BINARY_NAME = esh

.PHONY: all build test check audit outdated clean help tools update

all: check build test ## Run checks, then build and test

build: ## Build the project in debug mode
	cargo build

release: ## Build the project in release mode
	cargo build --release

test: ## Run tests
	cargo test

check: ## Run clippy and check formatting
	cargo clippy -- -D warnings
	cargo fmt --all --check

fix: ## Apply clippy recommendations and fmt fixes
	cargo fmt --all
	cargo clippy --fix --lib -p $(BINARY_NAME)

audit: ## Check for security vulnerabilities
	cargo audit

outdated: ## Check for out-of-date dependencies
	cargo outdated

update: ## Update dependencies to latest version
	cargo update

clean: ## Clean the build directory
	cargo clean

tools: ## Update the rust environment
	rustup update
	cargo install cargo-outdated
	cargo install cargo-audit

help: ## Display this help screen
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'
