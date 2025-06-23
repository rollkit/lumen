.PHONY: all build test clean fmt lint run help

# Build configuration
CARGO = cargo
BINARY_NAME = rollkit-reth
TARGET_DIR = target

# Default target
all: build

## help: Display this help message
help:
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*##"; printf "\n"} /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Building

## build: Build the rollkit-reth binary in release mode
build:
	$(CARGO) build --release --bin $(BINARY_NAME)

## build-dev: Build the rollkit-reth binary in debug mode
build-dev:
	$(CARGO) build --bin $(BINARY_NAME)

##@ Testing

## test: Run all tests
test:
	$(CARGO) test --all

## test-verbose: Run all tests with verbose output
test-verbose:
	$(CARGO) test --all -- --nocapture

## test-unit: Run unit tests only
test-unit:
	$(CARGO) test --lib

## test-integration: Run integration tests only  
test-integration:
	$(CARGO) test --test '*'

##@ Development

## run: Run the rollkit-reth node with default settings
run: build-dev
	./$(TARGET_DIR)/debug/$(BINARY_NAME) node

## run-dev: Run with debug logs enabled
run-dev: build-dev
	RUST_LOG=debug ./$(TARGET_DIR)/debug/$(BINARY_NAME) node

## fmt: Format code using rustfmt
fmt:
	$(CARGO) fmt --all

## fmt-check: Check if code is formatted correctly
fmt-check:
	$(CARGO) fmt --all -- --check

## lint: Run clippy linter
lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

## check: Run cargo check
check:
	$(CARGO) check --all

##@ Maintenance

## clean: Clean build artifacts
clean:
	$(CARGO) clean

## update: Update dependencies
update:
	$(CARGO) update

## audit: Audit dependencies for security vulnerabilities
audit:
	$(CARGO) audit

##@ Documentation

## doc: Build documentation
doc:
	$(CARGO) doc --no-deps --open

## doc-all: Build documentation including dependencies
doc-all:
	$(CARGO) doc --open