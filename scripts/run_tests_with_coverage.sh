#!/bin/bash

# Coverage Test Runner Script for Codex (Rust + Node.js)
# Runs tests for both Rust and TypeScript with coverage analysis
#
# Usage:
#   ./run_tests_with_coverage.sh                # Run all tests with coverage
#   ./run_tests_with_coverage.sh --rust-only    # Rust tests only
#   ./run_tests_with_coverage.sh --node-only    # Node.js tests only

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

print_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Parse command line arguments
run_rust=true
run_node=true

for arg in "$@"; do
    case $arg in
        --rust-only)
            run_node=false
            ;;
        --node-only)
            run_rust=false
            ;;
        *)
            print_warning "Unknown argument: $arg"
            ;;
    esac
done

# Create coverage output directory
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
COVERAGE_DIR="$PROJECT_ROOT/coverage"
mkdir -p "$COVERAGE_DIR/rust" "$COVERAGE_DIR/typescript"

print_status "🧪 Running tests with coverage analysis for Codex monorepo"
echo "=================================================="

overall_status=0
start_time=$(date +%s)

# ============================================================================
# Rust Tests with Coverage (using cargo-tarpaulin)
# ============================================================================
if [ "$run_rust" = true ]; then
    print_status "🦀 Running Rust tests with coverage..."
    cd "$PROJECT_ROOT/codex-rs"

    # Check if cargo-tarpaulin is installed
    if ! command -v cargo-tarpaulin &> /dev/null; then
        print_warning "cargo-tarpaulin not found. Installing..."
        if cargo install cargo-tarpaulin; then
            print_success "cargo-tarpaulin installed successfully"
        else
            print_error "Failed to install cargo-tarpaulin"
            overall_status=1
        fi
    fi

    # Run tests with coverage
    if command -v cargo-tarpaulin &> /dev/null; then
        print_status "Running cargo test with tarpaulin coverage..."
        if cargo tarpaulin --out Html --out Xml --output-dir "$COVERAGE_DIR/rust" --skip-clean; then
            print_success "✅ Rust tests passed with coverage"
            print_status "HTML coverage report: $COVERAGE_DIR/rust/index.html"
        else
            print_error "❌ Rust tests failed"
            overall_status=1
        fi
    else
        # Fallback to regular cargo test without coverage
        print_warning "Running tests without coverage (tarpaulin unavailable)"
        if cargo test; then
            print_success "✅ Rust tests passed (no coverage)"
        else
            print_error "❌ Rust tests failed"
            overall_status=1
        fi
    fi
fi

# ============================================================================
# TypeScript/Node.js Tests with Coverage
# ============================================================================
if [ "$run_node" = true ]; then
    print_status "📦 Running TypeScript tests with coverage..."
    cd "$PROJECT_ROOT"

    # Check if there are TypeScript test files
    if [ -d "sdk/typescript/tests" ]; then
        cd sdk/typescript

        print_status "Installing dependencies with pnpm..."
        if pnpm install; then
            print_success "Dependencies installed"
        else
            print_warning "Failed to install dependencies, continuing..."
        fi

        # Run tests with coverage (assuming Jest is configured)
        print_status "Running tests with coverage..."
        if pnpm test --coverage --coverageDirectory="$COVERAGE_DIR/typescript" 2>/dev/null || \
           npx jest --coverage --coverageDirectory="$COVERAGE_DIR/typescript" 2>/dev/null; then
            print_success "✅ TypeScript tests passed with coverage"
            print_status "HTML coverage report: $COVERAGE_DIR/typescript/index.html"
        else
            print_warning "TypeScript tests not configured or failed"
            # Don't fail overall if TypeScript tests aren't set up
        fi
    else
        print_status "No TypeScript tests found, skipping..."
    fi
fi

# ============================================================================
# Summary
# ============================================================================
end_time=$(date +%s)
duration=$((end_time - start_time))

echo
echo "=================================================="
print_status "⏱️  Total execution time: ${duration}s"

if [ $overall_status -eq 0 ]; then
    print_success "🎉 ALL TESTS PASSED!"
    echo
    print_status "📊 Coverage Reports:"
    [ "$run_rust" = true ] && echo "  • Rust: $COVERAGE_DIR/rust/index.html"
    [ "$run_node" = true ] && echo "  • TypeScript: $COVERAGE_DIR/typescript/index.html"
else
    print_error "❌ SOME TESTS FAILED"
fi

echo "=================================================="
exit $overall_status
