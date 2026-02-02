# OMG Makefile - Development and Testing Targets

.PHONY: help build release test check fmt clippy clean bench bench-fast bench-hyperfine bench-hyperfine-fast bench-charts docker-debian docker-ubuntu docker-test install audit dev

# Default target - show help
.DEFAULT_GOAL := help

# Display help information
help:
	@echo "OMG Development Makefile"
	@echo "========================"
	@echo ""
	@echo "Building:"
	@echo "  make build           - Development build"
	@echo "  make release         - Optimized release build"
	@echo "  make install         - Install to ~/.local/bin"
	@echo ""
	@echo "Testing:"
	@echo "  make test            - Run all tests"
	@echo "  make test-lib        - Run library tests only"
	@echo "  make tdd             - Watch mode (requires cargo-watch)"
	@echo "  make coverage        - Generate coverage report"
	@echo ""
	@echo "Quality:"
	@echo "  make check           - Fast check without building"
	@echo "  make fmt             - Format code"
	@echo "  make fmt-check       - Check formatting"
	@echo "  make clippy          - Run clippy linter"
	@echo "  make clippy-strict   - Clippy with pedantic+nursery"
	@echo "  make audit           - Security audit dependencies"
	@echo "  make qa              - Run all quality checks"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make bench           - Full benchmark suite"
	@echo "  make bench-fast      - Quick benchmark"
	@echo "  make bench-hyperfine - Hyperfine benchmark"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-test     - Test on Debian+Ubuntu"
	@echo "  make docker-debian   - Test on Debian"
	@echo "  make docker-ubuntu   - Test on Ubuntu"
	@echo ""
	@echo "Development:"
	@echo "  make dev             - Run development build with daemon"
	@echo "  make clean           - Clean build artifacts"

# Quick development target
all: build

# ═══════════════════════════════════════════════════════════════════════════════
# Building
# ═══════════════════════════════════════════════════════════════════════════════

# Development build
build:
	cargo build --features arch

# Release build with optimizations
release:
	cargo build --release --features arch

# Install to ~/.local/bin
install: release
	mkdir -p ~/.local/bin
	cp target/release/omg ~/.local/bin/
	cp target/release/omgd ~/.local/bin/
	@echo "✓ Installed omg and omgd to ~/.local/bin"

# ═══════════════════════════════════════════════════════════════════════════════
# Testing
# ═══════════════════════════════════════════════════════════════════════════════

# Run all tests
test:
	cargo test --features arch

# Run library tests only (fast)
test-lib:
	cargo test --lib --features arch

# ═══════════════════════════════════════════════════════════════════════════════
# Quality Checks
# ═══════════════════════════════════════════════════════════════════════════════

# Check without building (fast)
check:
	cargo check --features arch

# Format code
fmt:
	cargo fmt

# Check formatting without modifying
fmt-check:
	cargo fmt -- --check

# Run clippy with warnings as errors
clippy:
	cargo clippy --features arch -- -D warnings

# Run clippy with pedantic and nursery lints
clippy-strict:
	cargo clippy --features arch --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery

# Security audit dependencies
audit:
	cargo audit

# Run all quality checks
qa: fmt-check clippy-strict test-lib
	@echo "✓ All quality checks passed!"

# Clean build artifacts
clean:
	cargo clean

# ═══════════════════════════════════════════════════════════════════════════════
# TDD and Coverage
# ═══════════════════════════════════════════════════════════════════════════════

# Continuous testing (TDD mode)
tdd:
	cargo watch -x test

# Generate coverage report (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --ignore-config --ignore-tests --out html

# ═══════════════════════════════════════════════════════════════════════════════
# Benchmarking
# ═══════════════════════════════════════════════════════════════════════════════

# Run full benchmark suite (10 iterations, 2 warmup)
bench:
	./benchmark.sh

# Run fast benchmark (5 iterations, 1 warmup)
bench-fast:
	./benchmark.sh --fast

# Run hyperfine benchmark (requires hyperfine: pacman -S hyperfine)
bench-hyperfine:
	./benchmark-hyperfine.sh

# Run hyperfine benchmark in fast mode
bench-hyperfine-fast:
	./benchmark-hyperfine.sh --fast

# Generate benchmark visualization charts
bench-charts:
	python3 scripts/generate-benchmark-chart.py

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

# ═══════════════════════════════════════════════════════════════════════════════
# Development Workflow
# ═══════════════════════════════════════════════════════════════════════════════

# Run development environment (build + daemon)
dev: build
	@echo "Starting OMG daemon..."
	@pkill omgd 2>/dev/null || true
	./target/debug/omgd &
	@echo "Daemon started. Run 'make dev-stop' to stop."

# Stop development daemon
dev-stop:
	@pkill omgd || echo "No daemon running"

# Full development cycle: format, check, test
dev-check: fmt check test-lib
	@echo "✓ Development checks passed!"
