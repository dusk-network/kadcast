help: ## Display this help screen
	@grep -h -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

test: ## Run tests
	@cargo test --release

clippy: ## Run clippy
	@cargo clippy --all-features --release -- -D warnings

fmt: ## Format code (requires nightly)
	@cargo +nightly fmt --all

doc: ## Generate documentation
	@cargo doc --no-deps

clean: ## Clean build artifacts
	@cargo clean

.PHONY: help test clippy fmt doc clean
