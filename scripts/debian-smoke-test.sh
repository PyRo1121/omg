#!/bin/bash
set -e

# Build the project first
echo "Building OMG..."
cargo build --release --features debian

# Create a temporary Dockerfile for testing
cat <<EOF > Dockerfile.test
FROM ubuntu:24.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    libapt-pkg-dev \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY target/release/omg /usr/local/bin/omg
COPY target/release/omgd /usr/local/bin/omgd

# Set environment variables
ENV RUST_LOG=debug
ENV OMG_DAEMON_DATA_DIR=/var/lib/omg

# Create necessary directories
RUN mkdir -p /var/lib/omg

# Entrypoint script
COPY scripts/test-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
EOF

# Create entrypoint script
cat <<EOF > scripts/test-entrypoint.sh
#!/bin/bash
set -e

echo "Starting OMG Daemon..."
omgd & 
DAEMON_PID=$!

# Wait for daemon to start
sleep 2

echo "Running tests..."

echo "1. Search for 'vim'"
omg search vim > /tmp/search.log
if grep -q "vim" /tmp/search.log; then
    echo "✅ Search passed"
else
    echo "❌ Search failed"
    cat /tmp/search.log
    exit 1
fi

echo "2. Check status"
omg status > /tmp/status.log
if grep -q "Packages" /tmp/status.log; then
    echo "✅ Status passed"
else
    echo "❌ Status failed"
    cat /tmp/status.log
    exit 1
fi

echo "Stopping daemon..."
kill $DAEMON_PID
EOF

chmod +x scripts/test-entrypoint.sh

# Build and run Docker image
echo "Building Docker image..."
docker build -t omg-debian-test -f Dockerfile.test .

echo "Running integration tests..."
docker run --rm omg-debian-test

# Cleanup
rm Dockerfile.test scripts/test-entrypoint.sh
echo "Tests completed successfully!"