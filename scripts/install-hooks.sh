#!/bin/bash
#
# Install Git hooks for Tarbox project
# Run this script after cloning the repository: ./scripts/install-hooks.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

echo "📦 Installing Git hooks for Tarbox..."
echo ""

# Check if we're in a git repository
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    echo "❌ Error: Not a git repository. Please run this from the project root."
    exit 1
fi

# Create pre-commit hook
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/bash
#
# Pre-commit hook for Tarbox project
# Runs cargo fmt and cargo clippy before allowing commit
#

set -e

echo "🔍 Running pre-commit checks..."
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo not found. Please install Rust."
    exit 1
fi

# Run cargo fmt check
echo "📝 Checking code formatting (cargo fmt)..."
if ! cargo fmt --all -- --check; then
    echo ""
    echo "❌ Code formatting check failed!"
    echo "💡 Run 'cargo fmt --all' to fix formatting issues."
    exit 1
fi
echo "✅ Formatting check passed"
echo ""

# Run clippy
echo "🔧 Running linter (cargo clippy)..."
if ! cargo clippy --all-targets --all-features -- -D warnings 2>&1 | grep -v "^$"; then
    echo ""
    echo "❌ Clippy check failed!"
    echo "💡 Fix the warnings above before committing."
    exit 1
fi
echo "✅ Clippy check passed"
echo ""

echo "✨ All pre-commit checks passed! Proceeding with commit..."
exit 0
EOF

# Make hooks executable
chmod +x "$HOOKS_DIR/pre-commit"

echo "✅ Git hooks installed successfully!"
echo ""
echo "Installed hooks:"
echo "  - pre-commit: Runs cargo fmt and cargo clippy"
echo ""
echo "To bypass hooks (not recommended), use: git commit --no-verify"
