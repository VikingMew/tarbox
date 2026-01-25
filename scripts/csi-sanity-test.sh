#!/bin/bash
set -e

# CSI Sanity Test Script
# Tests Tarbox CSI driver compliance using csi-sanity tool

# Configuration
CSI_SOCKET="${CSI_SOCKET:-/tmp/csi-test.sock}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/tarbox}"
CSI_SANITY_VERSION="${CSI_SANITY_VERSION:-v5.2.0}"
TARBOX_BIN="${TARBOX_BIN:-./target/release/tarbox}"

echo "=== Tarbox CSI Sanity Test ==="
echo "Socket: $CSI_SOCKET"
echo "Database: $DATABASE_URL"
echo ""

# Check if csi-sanity is installed
if ! command -v csi-sanity &> /dev/null; then
    echo "Installing csi-sanity ${CSI_SANITY_VERSION}..."
    wget -q "https://github.com/kubernetes-csi/csi-test/releases/download/${CSI_SANITY_VERSION}/csi-sanity-${CSI_SANITY_VERSION}-linux-amd64.tar.gz"
    tar -xzf "csi-sanity-${CSI_SANITY_VERSION}-linux-amd64.tar.gz"
    sudo mv csi-sanity /usr/local/bin/
    rm "csi-sanity-${CSI_SANITY_VERSION}-linux-amd64.tar.gz"
    echo "✓ csi-sanity installed"
fi

# Build tarbox if needed
if [ ! -f "$TARBOX_BIN" ]; then
    echo "Building tarbox..."
    cargo build --release
    echo "✓ Build complete"
fi

# Cleanup previous runs
echo "Cleaning up..."
rm -f "$CSI_SOCKET"
pkill -f "tarbox csi" 2>/dev/null || true
sleep 1

# Start CSI server
echo "Starting CSI server..."
DATABASE_URL="$DATABASE_URL" "$TARBOX_BIN" csi \
    --mode=all \
    --endpoint="unix://$CSI_SOCKET" \
    --node-id=test-node &
CSI_PID=$!
sleep 3

# Verify server started
if ! kill -0 $CSI_PID 2>/dev/null; then
    echo "ERROR: CSI server failed to start"
    exit 1
fi
echo "✓ CSI server running (PID: $CSI_PID)"

# Run csi-sanity tests
echo ""
echo "Running csi-sanity tests..."
echo "Note: Skipping mount tests (no kubelet environment)"
echo ""

csi-sanity \
    --csi.endpoint="$CSI_SOCKET" \
    --csi.testvolumeparameters='csi.storage.k8s.io/pvc/namespace=default,csi.storage.k8s.io/pvc/name=sanity-test' \
    --ginkgo.skip="NodePublishVolume.*should work|NodeUnpublishVolume.*should work|NodeStageVolume.*should work|NodeUnstageVolume.*should work|should be idempotent" \
    --ginkgo.v \
    "$@"

TEST_RESULT=$?

# Cleanup
echo ""
echo "Cleaning up..."
kill $CSI_PID 2>/dev/null || true
rm -f "$CSI_SOCKET"

if [ $TEST_RESULT -eq 0 ]; then
    echo "✓ All tests passed!"
else
    echo "✗ Some tests failed"
fi

exit $TEST_RESULT
