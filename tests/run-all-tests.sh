#!/bin/bash
# SRT CLI End-to-End Test Suite
#
# Runs comprehensive tests to validate SRT multi-path bonding functionality

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "========================================="
echo "SRT CLI End-to-End Test Suite"
echo "========================================="
echo ""

# Change to project root
cd "$PROJECT_ROOT"

# Build binaries first
echo "Building SRT CLI tools..."
cargo build --release --bin srt-sender --bin srt-receiver 2>&1 | grep -E "(Compiling|Finished)" || true
echo ""

# Check if binaries exist
if [ ! -f "./target/release/srt-sender" ] || [ ! -f "./target/release/srt-receiver" ]; then
    echo -e "${RED}❌ ERROR: Binaries not found. Build failed.${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Binaries built successfully${NC}"
echo ""

# Test files
TESTS=(
    "01-perfect-conditions.sh"
    "02-lossy-conditions.sh"
    "03-multipath-bonding.sh"
    "04-streaming-simulation.sh"
)

# Results tracking
TOTAL_TESTS=${#TESTS[@]}
PASSED=0
FAILED=0
FAILED_TESTS=()

echo "========================================="
echo "Running $TOTAL_TESTS tests..."
echo "========================================="
echo ""

# Run each test
for test in "${TESTS[@]}"; do
    TEST_PATH="$SCRIPT_DIR/$test"

    if [ ! -f "$TEST_PATH" ]; then
        echo -e "${RED}❌ Test not found: $test${NC}"
        FAILED=$((FAILED + 1))
        FAILED_TESTS+=("$test (not found)")
        continue
    fi

    # Make test executable
    chmod +x "$TEST_PATH"

    # Run test and capture result
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${BLUE}Running: $test${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    if "$TEST_PATH"; then
        PASSED=$((PASSED + 1))
        echo ""
        echo -e "${GREEN}✅ PASSED: $test${NC}"
    else
        FAILED=$((FAILED + 1))
        FAILED_TESTS+=("$test")
        echo ""
        echo -e "${RED}❌ FAILED: $test${NC}"
    fi
done

# Final summary
echo ""
echo ""
echo "========================================="
echo "Test Suite Summary"
echo "========================================="
echo ""
echo "Total tests:  $TOTAL_TESTS"
echo -e "Passed:       ${GREEN}$PASSED${NC}"
echo -e "Failed:       ${RED}$FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}✅ ALL TESTS PASSED${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "The SRT CLI tools are working correctly!"
    echo ""
    echo "✓ Perfect conditions: 100% delivery"
    echo "✓ Lossy conditions: Acceptable degradation"
    echo "✓ Multi-path bonding: Working correctly"
    echo "✓ Streaming: MPEGTS-compatible delivery"
    echo ""
    EXIT_CODE=0
else
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}❌ SOME TESTS FAILED${NC}"
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "Failed tests:"
    for failed_test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}✗${NC} $failed_test"
    done
    echo ""
    echo "Check individual test output above for details."
    echo ""
    EXIT_CODE=1
fi

echo "Test artifacts saved in /tmp/srt-test-*"
echo ""

exit $EXIT_CODE
