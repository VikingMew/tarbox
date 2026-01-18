#!/bin/bash
# Run E2E tests locally
# Usage: ./scripts/run-e2e-tests.sh [--with-fuse]

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default settings
WITH_FUSE=false
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/tarbox_test}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-fuse)
            WITH_FUSE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [--with-fuse]"
            echo ""
            echo "Options:"
            echo "  --with-fuse    Run FUSE mount tests (requires sudo)"
            echo "  --help         Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

echo -e "${GREEN}=== Tarbox E2E Test Suite ===${NC}"
echo ""

# Check PostgreSQL
echo -e "${YELLOW}Checking PostgreSQL...${NC}"
if ! psql "$DATABASE_URL" -c '\q' 2>/dev/null; then
    echo -e "${RED}Error: Cannot connect to PostgreSQL${NC}"
    echo "Please ensure PostgreSQL is running and DATABASE_URL is correct:"
    echo "  export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox_test"
    exit 1
fi
echo -e "${GREEN}✓ PostgreSQL connection OK${NC}"
echo ""

# Run migrations
echo -e "${YELLOW}Running database migrations...${NC}"
if ! command -v sqlx &> /dev/null; then
    echo "Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features postgres
fi

sqlx database create 2>/dev/null || true
sqlx migrate run
echo -e "${GREEN}✓ Migrations OK${NC}"
echo ""

# Run unit tests
echo -e "${YELLOW}Running unit tests...${NC}"
cargo test --lib
echo -e "${GREEN}✓ Unit tests passed${NC}"
echo ""

# Run E2E tests (without FUSE)
echo -e "${YELLOW}Running E2E tests...${NC}"

echo "  - FileSystem integration tests..."
cargo test --test filesystem_integration_test

echo "  - FuseBackend integration tests..."
cargo test --test fuse_backend_integration_test

echo "  - Storage E2E tests..."
cargo test --test storage_e2e_test

echo -e "${GREEN}✓ E2E tests passed${NC}"
echo ""

# Run FUSE mount tests if requested
if [ "$WITH_FUSE" = true ]; then
    echo -e "${YELLOW}Running FUSE mount E2E tests...${NC}"

    # Check FUSE
    if ! command -v fusermount &> /dev/null && ! command -v fusermount3 &> /dev/null; then
        echo -e "${RED}Error: FUSE not installed${NC}"
        echo "Install with: sudo apt-get install fuse3 libfuse3-dev"
        exit 1
    fi

    # Check if user is in fuse group or is root
    if ! groups | grep -q fuse && [ "$EUID" -ne 0 ]; then
        echo -e "${YELLOW}Warning: Running FUSE tests requires sudo or fuse group membership${NC}"
        echo "Running with sudo..."
        sudo -E DATABASE_URL="$DATABASE_URL" $(which cargo) test --test fuse_mount_e2e_test -- --ignored --test-threads=1
    else
        cargo test --test fuse_mount_e2e_test -- --ignored --test-threads=1
    fi

    echo -e "${GREEN}✓ FUSE mount tests passed${NC}"
    echo ""
fi

# Generate coverage report
echo -e "${YELLOW}Generating coverage report...${NC}"

if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "Installing cargo-llvm-cov..."
    cargo install cargo-llvm-cov
fi

cargo llvm-cov clean

# Run coverage for lib and E2E tests
cargo llvm-cov --lib --test filesystem_integration_test --test fuse_backend_integration_test --test storage_e2e_test

echo ""
echo -e "${GREEN}=== All tests passed! ===${NC}"

# Check coverage threshold
COVERAGE=$(cargo llvm-cov --lib --test filesystem_integration_test --test fuse_backend_integration_test --test storage_e2e_test --summary-only 2>/dev/null | grep "^TOTAL" | awk '{print $10}' | tr -d '%')

if (( $(echo "$COVERAGE >= 80" | bc -l) )); then
    echo -e "${GREEN}✓ Coverage: ${COVERAGE}% (target: 80%)${NC}"
else
    echo -e "${YELLOW}⚠ Coverage: ${COVERAGE}% (target: 80%)${NC}"
    if [ "$WITH_FUSE" = false ]; then
        echo -e "${YELLOW}  Hint: Run with --with-fuse to include FUSE mount tests${NC}"
    fi
fi
