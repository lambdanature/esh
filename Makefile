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

test-quiet: ## Run tests, quietly
	cargo nextest run --status-level fail

test: ## Run tests
	cargo nextest run

test-release: ## Run tests in release mode
	cargo nextest run --release

check-precommit:
	@diff -uN .git/hooks/pre-commit .git-pre-commit-template || ( \
          echo "+-------------------------------------------------------+"; \
          echo "| ERROR: pre-commit outdated, 'make init-git' to update |"; \
          echo "+-------------------------------------------------------+"; \
	   exit 1)
	@echo "pre-commit: ✅ Up to date: .git/hooks/pre-commit (.git-pre-commit-template)"

precommit: check-precommit
	.git/hooks/pre-commit

check: check-precommit ## Check precommit, run clippy and check formatting
	cargo clippy --all-targets --all-features
	cargo fmt --all --check

fix: ## Apply clippy recommendations and fmt fixes
	cargo fmt --all
	cargo clippy --fix --lib -p $(NAME)

audit: ## Check for security vulnerabilities
	cargo audit

coverage:
	cargo tarpaulin

outdated: ## Check for out-of-date dependencies
	cargo outdated

update: ## Update dependencies to latest version
	cargo update

clean: ## Clean the build directory
	cargo clean

tools: ## Update the rust environment
	rustup update
	cargo install --locked cargo-outdated
	cargo install --locked cargo-audit
	cargo install --locked cargo-nextest
	cargo install --locked cargo-tarpaulin

init-git: ## Install precommit hooks
	install -m 755 .git-pre-commit-template .git/hooks/pre-commit

help: ## Display this help screen
	@echo; echo "  Welcome to $(NAME)-v$(VERSION), available targets:"; echo
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
	awk 'BEGIN {FS = ":.*?## "}; {printf "    %-14s %s\n", $$1, $$2}'
	@echo
