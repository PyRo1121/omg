# OMG Makefile - Development and Testing Targets

.PHONY: build release test check fmt clippy clean docker-debian docker-ubuntu docker-test

# Default target
all: build

# Development build
build:
	cargo build

# Release build
release:
	cargo build --release

# Run tests
test:
	cargo test

# Check without building
check:
	cargo check

# Format code
fmt:
	cargo fmt

# Run clippy
clippy:
	cargo clippy

# Clean build artifacts
clean:
	cargo clean

# ═══════════════════════════════════════════════════════════════════════════════
# Docker Testing (for Debian/Ubuntu support development on Arch)
# ═══════════════════════════════════════════════════════════════════════════════

# Build and test on Debian Bookworm
docker-debian:
	@echo "Building OMG for Debian Bookworm..."
	docker build -f Dockerfile.debian -t omg-debian .
	@echo "Running smoke tests..."
	docker run --rm omg-debian

# Build and test on Ubuntu 24.04
docker-ubuntu:
	@echo "Building OMG for Ubuntu 24.04..."
	docker build -f Dockerfile.ubuntu -t omg-ubuntu .
	@echo "Running smoke tests..."
	docker run --rm omg-ubuntu

# Run both Debian and Ubuntu tests
docker-test: docker-debian docker-ubuntu
	@echo ""
	@echo "════════════════════════════════════════════════════════════════"
	@echo "  All Docker tests passed! ✓"
	@echo "════════════════════════════════════════════════════════════════"

# Interactive shell in Debian container (for debugging)
docker-debian-shell:
	docker build -f Dockerfile.debian -t omg-debian .
	docker run --rm -it omg-debian /bin/bash

# Interactive shell in Ubuntu container (for debugging)
docker-ubuntu-shell:
	docker build -f Dockerfile.ubuntu -t omg-ubuntu .
	docker run --rm -it omg-ubuntu /bin/bash
