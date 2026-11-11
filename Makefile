# devine-utils-ci — development tasks.
#
# The `blaze` targets use scripts/blaze.sh, which works around the sandbox
# restrictions of containers without user namespaces.

CARGO ?= cargo
BLAZE ?= scripts/blaze.sh
TARGET_DIR ?= target
BIN := $(TARGET_DIR)/debug/devine-utils-ci
RELEASE_BIN := $(TARGET_DIR)/release/devine-utils-ci

.PHONY: all build release test lint fmt fmt-check clippy doc check gate gates rust-gates clean blaze-build blaze-test help

all: build

## build: compile the debug binary
build:
	$(CARGO) build --all-targets

## release: compile the release binary
release:
	$(CARGO) build --release --all-targets

## test: run the unit and integration tests
test:
	$(CARGO) test --all-targets

## lint: fmt-check + clippy (what CI runs before the gates)
lint: fmt-check clippy

## fmt: rewrite the sources with rustfmt
fmt:
	$(CARGO) fmt --all

## fmt-check: fail when the sources are not rustfmt-clean
fmt-check:
	$(CARGO) fmt --all -- --check

## clippy: run clippy with warnings denied
clippy:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

## doc: build the API documentation
doc:
	$(CARGO) doc --no-deps

## check: fmt-check + clippy + test
check: fmt-check clippy test

## gate: run every quality gate against this repository
gate: build
	$(BIN) all
	$(BIN) ci-go-env

## gates: run the language-agnostic gates only
gates: build
	$(BIN) bugs
	$(BIN) sanity
	$(BIN) license
	$(BIN) structure
	$(BIN) file-length
	$(BIN) complexity

## rust-gates: run the Rust gates only
rust-gates: build
	$(BIN) rust

## blaze-build: compile with blaze
blaze-build:
	$(BLAZE) build

## blaze-test: run the tests with blaze
blaze-test:
	$(BLAZE) test

## clean: remove the build artifacts
clean:
	$(CARGO) clean

## help: list the available targets
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/^## //'
