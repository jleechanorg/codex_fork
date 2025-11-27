#!/bin/bash

# Comprehensive Linting Script for Codex (Rust + Node.js)
# Runs cargo fmt, cargo clippy for Rust and Prettier for Node.js

set -euo pipefail  # Exit on any command failure, treat unset variables as errors, and catch pipeline failures

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Running comprehensive linting for Codex monorepo${NC}"
echo "=================================================="

# Track overall status
overall_status=0

# Function to run a linter with proper error handling
run_linter() {
    local tool_name="$1"
    local command="$2"
    local emoji="$3"

    echo -e "\n${BLUE}${emoji} Running ${tool_name}...${NC}"
    echo "Command: $command"

    if eval "$command"; then
        echo -e "${GREEN}✅ ${tool_name}: PASSED${NC}"
        return 0
    else
        echo -e "${RED}❌ ${tool_name}: FAILED${NC}"
        return 1
    fi
}

# 1. Rust Linting (Clippy)
echo -e "\n${BLUE}🦀 STEP 1: Rust Linting (cargo clippy)${NC}"
cd codex-rs
clippy_cmd="cargo clippy --all-targets --all-features -- -D warnings"

if ! run_linter "cargo clippy" "$clippy_cmd" "🦀"; then
    overall_status=1
fi

# 2. Rust Formatting Check
echo -e "\n${BLUE}🎨 STEP 2: Rust Formatting (cargo fmt)${NC}"
fmt_cmd="cargo fmt -- --config imports_granularity=Item --check"

if ! run_linter "cargo fmt" "$fmt_cmd" "🎨"; then
    overall_status=1
fi

# 3. Node.js Formatting (Prettier)
echo -e "\n${BLUE}📋 STEP 3: Node.js Formatting (Prettier)${NC}"
cd ..
prettier_cmd="pnpm run format"

if ! run_linter "Prettier" "$prettier_cmd" "📋"; then
    overall_status=1
fi

# Summary
echo -e "\n=================================================="
if [[ $overall_status -eq 0 ]]; then
    echo -e "${GREEN}🎉 ALL LINTING CHECKS PASSED!${NC}"
    echo -e "${GREEN}✅ Rust (clippy + fmt) and Node.js (Prettier) all successful${NC}"
else
    echo -e "${RED}❌ SOME LINTING CHECKS FAILED${NC}"
    echo -e "${YELLOW}💡 To auto-fix formatting issues:${NC}"
    echo -e "${YELLOW}   Rust: cd codex-rs && cargo fmt${NC}"
    echo -e "${YELLOW}   Node: pnpm run format:fix${NC}"
fi

echo -e "\n${BLUE}📊 Linting Summary:${NC}"
echo "  • Rust: cargo clippy + cargo fmt"
echo "  • Node.js: Prettier"
echo "  • Status: $([ $overall_status -eq 0 ] && echo "PASSED" || echo "FAILED")"

exit $overall_status
