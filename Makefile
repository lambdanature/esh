# Variables
NAME=$(shell awk -F' *= *' '/^name/ {gsub(/"/, "", $$2); print $$2; exit}' Cargo.toml)
VERSION=$(shell awk -F' *= *' '/^version/ {gsub(/"/, "", $$2); print $$2; exit}' Cargo.toml)

.PHONY: all build test check audit outdated clean help tools update init-git
.PHONY: check-precommit test-release

all: check audit build test ## Run checks, audit, then build and test

build: ## Build the project in debug mode
	cargo build

release: ## Build the project in release mode
	cargo build --release

test: ## Run tests
	cargo test

test-release: ## Run tests in release mode
	cargo test --release

check-precommit:
	@diff -uN .git/hooks/pre-commit .git-pre-commit-template || ( \
          echo "+-------------------------------------------------------+"; \
          echo "| ERROR: pre-commit outdated, 'make init-git' to update |"; \
          echo "+-------------------------------------------------------+"; \
	   exit 1)
	@echo "Up to date: .git/hooks/pre-commit (.git-pre-commit-template)"

check: check-precommit ## Check precommit, run clippy and check formatting
	cargo clippy -- -D warnings
	cargo fmt --all --check

fix: ## Apply clippy recommendations and fmt fixes
	cargo fmt --all
	cargo clippy --fix --lib -p $(NAME)

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

init: ## Install precommit hooks
	install -m 755 .git-pre-commit-template .git/hooks/pre-commit

help: ## Display this help screen
	@echo; echo "  Welcome to $(NAME)-v$(VERSION), available targets:"; echo
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
	awk 'BEGIN {FS = ":.*?## "}; {printf "    %-14s %s\n", $$1, $$2}'
	@echo
