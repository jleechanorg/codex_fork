#!/bin/bash

# Dedicated Coverage Report Script for Codex (Rust + Node.js)
# Displays and aggregates coverage reports from both Rust and TypeScript
#
# Usage:
#   ./coverage.sh                # Display existing coverage reports
#   ./coverage.sh --generate     # Generate fresh coverage reports

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

# Get project root
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
COVERAGE_DIR="$PROJECT_ROOT/coverage"

# Parse arguments
generate=false
for arg in "$@"; do
    case $arg in
        --generate)
            generate=true
            ;;
        *)
            print_warning "Unknown argument: $arg"
            ;;
    esac
done

echo "=================================================="
print_status "📊 Coverage Report for Codex Monorepo"
echo "=================================================="

# Generate coverage if requested
if [ "$generate" = true ]; then
    print_status "Generating fresh coverage reports..."
    if "$PROJECT_ROOT/scripts/run_tests_with_coverage.sh"; then
        print_success "Coverage generation complete"
    else
        print_error "Coverage generation failed"
        exit 1
    fi
    echo
fi

# Check for Rust coverage
rust_coverage_found=false
if [ -f "$COVERAGE_DIR/rust/index.html" ]; then
    rust_coverage_found=true
    print_success "Rust coverage report found"
    echo "  Location: $COVERAGE_DIR/rust/index.html"

    # Try to extract coverage percentage from tarpaulin output
    if [ -f "$COVERAGE_DIR/rust/cobertura.xml" ]; then
        rust_coverage=$(grep -o 'line-rate="[0-9.]*"' "$COVERAGE_DIR/rust/cobertura.xml" | head -1 | grep -o '[0-9.]*')
        if [ -n "$rust_coverage" ]; then
            rust_pct=$(echo "$rust_coverage * 100" | bc -l)
            printf "  Coverage: %.2f%%\n" "$rust_pct"
        fi
    fi
else
    print_warning "Rust coverage not found"
    echo "  Run: ./scripts/run_tests_with_coverage.sh --rust-only"
fi

echo

# Check for TypeScript coverage
ts_coverage_found=false
if [ -f "$COVERAGE_DIR/typescript/index.html" ]; then
    ts_coverage_found=true
    print_success "TypeScript coverage report found"
    echo "  Location: $COVERAGE_DIR/typescript/index.html"

    # Try to extract coverage from lcov-report
    if [ -f "$COVERAGE_DIR/typescript/coverage-summary.json" ]; then
        ts_coverage=$(jq -r '.total.lines.pct' "$COVERAGE_DIR/typescript/coverage-summary.json" 2>/dev/null || echo "")
        if [ -n "$ts_coverage" ]; then
            echo "  Coverage: ${ts_coverage}%"
        fi
    fi
else
    print_warning "TypeScript coverage not found"
    echo "  Run: ./scripts/run_tests_with_coverage.sh --node-only"
fi

echo
echo "=================================================="

# Provide helpful commands
if [ "$rust_coverage_found" = false ] || [ "$ts_coverage_found" = false ]; then
    echo
    print_status "💡 Quick commands:"
    if [ "$rust_coverage_found" = false ]; then
        echo "  Generate Rust coverage:       ./scripts/run_tests_with_coverage.sh --rust-only"
    fi
    if [ "$ts_coverage_found" = false ]; then
        echo "  Generate TypeScript coverage: ./scripts/run_tests_with_coverage.sh --node-only"
    fi
    echo "  Generate all coverage:        ./scripts/run_tests_with_coverage.sh"
    echo "  View this report:             ./scripts/coverage.sh"
fi

# Open reports in browser (macOS/Linux)
if [ "$rust_coverage_found" = true ] || [ "$ts_coverage_found" = true ]; then
    echo
    print_status "🌐 Open coverage reports:"
    if [ "$rust_coverage_found" = true ]; then
        echo "  Rust:       open $COVERAGE_DIR/rust/index.html"
    fi
    if [ "$ts_coverage_found" = true ]; then
        echo "  TypeScript: open $COVERAGE_DIR/typescript/index.html"
    fi
fi

echo "=================================================="

# Exit with success if at least one coverage report exists
if [ "$rust_coverage_found" = true ] || [ "$ts_coverage_found" = true ]; then
    exit 0
else
    print_warning "No coverage reports found. Run with --generate to create them."
    exit 1
fi
