#!/bin/bash

# ZeroCache Benchmark Script - Primary Key Lookup Only
# Usage: ./benchmark.sh

set -e

# ============================================
# CONFIGURATION - Edit these values
# ============================================

# Server settings
HOST="http://127.0.0.1:8080"
COLLECTION="products"

# Test data - Use ONE objectID for consistent response size
TEST_OBJECT_ID=1  # Single objectID for all tests

# Concurrency levels to test
CONCURRENCY_LEVELS=(10 50 100 200 500)

# Number of requests per concurrency level
REQUESTS_PER_TEST=500000  # 500k requests per test

# Sustained load test duration (seconds)
SUSTAINED_LOAD_DURATION=120  # 2 minutes
SUSTAINED_LOAD_CONCURRENCY=100

# ============================================
# END CONFIGURATION
# ============================================

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
RESULTS_DIR="$SCRIPT_DIR/results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Log function
log() {
    echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

# Check if server is running
log "Checking if ZeroCache is running on $HOST..."
if ! curl -s "$HOST/status" > /dev/null 2>&1; then
    error "ZeroCache is not running on $HOST"
    echo "Please start the server first: ./target/release/zerocache"
    exit 1
fi
success "Server is running"
echo ""

# Get server settings
log "Server settings:"
SETTINGS=$(curl -s "$HOST/settings")
echo "$SETTINGS" | jq '{rate_limit_per_second, allowed_ips}' 2>/dev/null || echo "$SETTINGS"

# Check rate limit
RATE_LIMIT=$(echo "$SETTINGS" | jq -r '.rate_limit_per_second // 10' 2>/dev/null)
if [ "$RATE_LIMIT" != "0" ] && [ "$RATE_LIMIT" != "null" ]; then
    warn "Rate limit is set to $RATE_LIMIT req/sec"
    echo "For accurate benchmarks, set rate_limit_per_second to 0"
    echo ""
fi

# Verify test objectID exists
log "Verifying test objectID=$TEST_OBJECT_ID exists..."
RESPONSE=$(curl -s "$HOST/data/$COLLECTION?objectID=$TEST_OBJECT_ID")
if echo "$RESPONSE" | grep -q "\"total\":0"; then
    error "ObjectID $TEST_OBJECT_ID not found in collection $COLLECTION"
    echo "Available objectIDs:"
    curl -s "$HOST/data/$COLLECTION?limit=10" | jq -r '.products[].objectID' 2>/dev/null || echo "Could not fetch objectIDs"
    exit 1
fi
success "ObjectID $TEST_OBJECT_ID found"

# Check response size
RESPONSE_SIZE=$(echo "$RESPONSE" | wc -c)
log "Response size: $RESPONSE_SIZE bytes"
echo ""

# Summary file
SUMMARY_FILE="$RESULTS_DIR/summary.txt"
echo "ZeroCache Benchmark Results - Primary Key Lookup" > "$SUMMARY_FILE"
echo "Generated: $(date)" >> "$SUMMARY_FILE"
echo "==========================================" >> "$SUMMARY_FILE"
echo "Configuration:" >> "$SUMMARY_FILE"
echo "  Host: $HOST" >> "$SUMMARY_FILE"
echo "  Collection: $COLLECTION" >> "$SUMMARY_FILE"
echo "  Test ObjectID: $TEST_OBJECT_ID" >> "$SUMMARY_FILE"
echo "  Response Size: $RESPONSE_SIZE bytes" >> "$SUMMARY_FILE"
echo "  Rate Limit: $RATE_LIMIT req/sec" >> "$SUMMARY_FILE"
echo "  Requests per test: $REQUESTS_PER_TEST" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"

# Function to run benchmark and extract key metrics
run_benchmark() {
    local test_name="$1"
    local url="$2"
    local requests="$3"
    local concurrency="$4"
    local output_file="$RESULTS_DIR/${test_name// /_}.txt"

    log "Running: $test_name"
    echo "  URL: $url"
    echo "  Requests: $requests | Concurrency: $concurrency"

    # Run ab and save output
    ab -n "$requests" -c "$concurrency" "$url" > "$output_file" 2>&1

    # Extract key metrics
    local rps=$(grep "Requests per second:" "$output_file" | awk '{print $4}')
    local time_per_req=$(grep "Time per request:" "$output_file" | head -1 | awk '{print $4}')
    local p50=$(grep "50%" "$output_file" | awk '{print $2}')
    local p95=$(grep "95%" "$output_file" | awk '{print $2}')
    local p99=$(grep "99%" "$output_file" | awk '{print $2}')
    local p100=$(grep "100%" "$output_file" | awk '{print $2}')
    local total_time=$(grep "Time taken for tests:" "$output_file" | awk '{print $5}')
    local complete=$(grep "Complete requests:" "$output_file" | awk '{print $3}')
    local failed=$(grep "Failed requests:" "$output_file" | awk '{print $3}')

    # Print results
    success "$test_name completed"
    echo "  RPS: ${GREEN}${rps}${NC} req/sec"
    echo "  Avg Latency: ${time_per_req}ms"
    echo "  Percentiles: p50=${p50}ms | p95=${p95}ms | p99=${p99}ms | p100=${p100}ms"
    echo "  Complete: $complete | Failed: $failed"
    echo ""

    # Save to summary
    echo "$test_name" >> "$SUMMARY_FILE"
    echo "  Requests: $requests | Concurrency: $concurrency" >> "$SUMMARY_FILE"
    echo "  Complete: $complete | Failed: $failed" >> "$SUMMARY_FILE"
    echo "  Requests/sec: $rps" >> "$SUMMARY_FILE"
    echo "  Time per request: ${time_per_req} ms" >> "$SUMMARY_FILE"
    echo "  Total time: ${total_time} seconds" >> "$SUMMARY_FILE"
    echo "  Percentiles: p50=${p50}ms, p95=${p95}ms, p99=${p99}ms, p100=${p100}ms" >> "$SUMMARY_FILE"
    echo "" >> "$SUMMARY_FILE"
}

# ============================================
# Run Primary Key Lookup Tests
# ============================================
echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}  Primary Key Lookup Benchmark${NC}"
echo -e "${YELLOW}  Testing objectID=$TEST_OBJECT_ID${NC}"
echo -e "${YELLOW}========================================${NC}"
echo ""

test_number=1
for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
    echo -e "${YELLOW}=== TEST $test_number: Concurrency = $concurrency ===${NC}"

    run_benchmark "Primary_Key_c${concurrency}" \
        "$HOST/data/$COLLECTION?objectID=$TEST_OBJECT_ID" \
        $REQUESTS_PER_TEST \
        $concurrency

    test_number=$((test_number + 1))
done

# ============================================
# Sustained Load Test
# ============================================
echo -e "${YELLOW}=== SUSTAINED LOAD TEST ===${NC}"
echo -e "${YELLOW}Duration: ${SUSTAINED_LOAD_DURATION}s | Concurrency: ${SUSTAINED_LOAD_CONCURRENCY}${NC}"
echo ""

log "Running sustained load test..."
ab -t $SUSTAINED_LOAD_DURATION -c $SUSTAINED_LOAD_CONCURRENCY \
    "$HOST/data/$COLLECTION?objectID=$TEST_OBJECT_ID" \
    > "$RESULTS_DIR/sustained_load.txt" 2>&1

rps=$(grep "Requests per second:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $4}')
total_req=$(grep "Complete requests:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $3}')
failed=$(grep "Failed requests:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $3}')
avg_time=$(grep "Time per request:" "$RESULTS_DIR/sustained_load.txt" | head -1 | awk '{print $4}')

success "Sustained Load Test completed"
echo "  Duration: ${SUSTAINED_LOAD_DURATION}s"
echo "  Total requests: ${total_req}"
echo "  Average RPS: ${GREEN}${rps}${NC}"
echo "  Avg Latency: ${avg_time}ms"
echo "  Failed: $failed"
echo ""

echo "Sustained Load Test (${SUSTAINED_LOAD_DURATION} seconds, c${SUSTAINED_LOAD_CONCURRENCY})" >> "$SUMMARY_FILE"
echo "  Total requests: $total_req" >> "$SUMMARY_FILE"
echo "  Failed requests: $failed" >> "$SUMMARY_FILE"
echo "  Average RPS: $rps" >> "$SUMMARY_FILE"
echo "  Average latency: $avg_time ms" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"

# ============================================
# Create CSV Report
# ============================================
CSV_FILE="$RESULTS_DIR/results.csv"
echo "Test,Concurrency,Requests,Complete,Failed,RPS,Avg_Latency_ms,p50_ms,p95_ms,p99_ms,p100_ms" > "$CSV_FILE"

for file in "$RESULTS_DIR"/*.txt; do
    if [[ "$file" != *"summary"* ]] && [[ "$file" != *"sustained"* ]]; then
        test_name=$(basename "$file" .txt | sed 's/_/ /g')

        concurrency=$(grep "Concurrency Level:" "$file" | awk '{print $3}')
        requests=$(grep "Complete requests:" "$file" | awk '{print $3}')
        complete=$(grep "Complete requests:" "$file" | awk '{print $3}')
        failed=$(grep "Failed requests:" "$file" | awk '{print $3}')
        rps=$(grep "Requests per second:" "$file" | awk '{print $4}')
        time=$(grep "Time per request:" "$file" | head -1 | awk '{print $4}')
        p50=$(grep "50%" "$file" | awk '{print $2}')
        p95=$(grep "95%" "$file" | awk '{print $2}')
        p99=$(grep "99%" "$file" | awk '{print $2}')
        p100=$(grep "100%" "$file" | awk '{print $2}')

        echo "$test_name,$concurrency,$requests,$complete,$failed,$rps,$time,$p50,$p95,$p99,$p100" >> "$CSV_FILE"
    fi
done

# Add sustained load to CSV
if [ -f "$RESULTS_DIR/sustained_load.txt" ]; then
    concurrency=$SUSTAINED_LOAD_CONCURRENCY
    requests=$(grep "Complete requests:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $3}')
    complete=$requests
    failed=$(grep "Failed requests:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $3}')
    rps=$(grep "Requests per second:" "$RESULTS_DIR/sustained_load.txt" | awk '{print $4}')
    time=$(grep "Time per request:" "$RESULTS_DIR/sustained_load.txt" | head -1 | awk '{print $4}')
    p50=$(grep "50%" "$RESULTS_DIR/sustained_load.txt" | awk '{print $2}')
    p95=$(grep "95%" "$RESULTS_DIR/sustained_load.txt" | awk '{print $2}')
    p99=$(grep "99%" "$RESULTS_DIR/sustained_load.txt" | awk '{print $2}')
    p100=$(grep "100%" "$RESULTS_DIR/sustained_load.txt" | awk '{print $2}')

    echo "Sustained Load,${concurrency},$requests,$complete,$failed,$rps,$time,$p50,$p95,$p99,$p100" >> "$CSV_FILE"
fi

# ============================================
# Final Summary
# ============================================
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}     Benchmark Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
log "Results saved to: $RESULTS_DIR/"
log "Summary: $SUMMARY_FILE"
log "CSV Report: $CSV_FILE"
echo ""

# Display summary
cat "$SUMMARY_FILE"
echo ""

# Create README
cat > "$RESULTS_DIR/README.txt" << EOF
ZeroCache Benchmark Results - Primary Key Lookup
Generated: $(date)

Test Configuration:
- Collection: $COLLECTION
- ObjectID tested: $TEST_OBJECT_ID
- Response size: $RESPONSE_SIZE bytes
- Requests per test: $REQUESTS_PER_TEST
- Concurrency levels: ${CONCURRENCY_LEVELS[@]}
- Sustained load: ${SUSTAINED_LOAD_DURATION}s @ c${SUSTAINED_LOAD_CONCURRENCY}

Files:
- summary.txt: Human-readable summary
- results.csv: CSV format for Excel/analysis
- *.txt: Detailed Apache Benchmark output for each test

Key Metrics:
- RPS (Requests Per Second): Throughput
- Latency: Response time in milliseconds
- Percentiles: p50 (median), p95, p99, p100 (max)

About "Failed requests":
If all tests show "Failed: 0" - Perfect! Response size is consistent.
If some tests show failed requests, it may be due to:
- Apache Benchmark artifacts at very high speeds
- Connection timeouts at extreme concurrency
As long as HTTP 200 responses are returned, the server is working correctly.

Focus on RPS and latency percentiles for performance evaluation.
EOF

success "README created: $RESULTS_DIR/README.txt"
echo ""

# Show quick summary table
echo -e "${BLUE}Quick Summary:${NC}"
echo "┌────────────┬──────────┬─────────────┬──────────────┐"
echo "│ Concurrency│   RPS    │ Avg Latency │     p99      │"
echo "├────────────┼──────────┼─────────────┼──────────────┤"

for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
    file="$RESULTS_DIR/Primary_Key_c${concurrency}.txt"
    if [ -f "$file" ]; then
        rps=$(grep "Requests per second:" "$file" | awk '{print $4}')
        time=$(grep "Time per request:" "$file" | head -1 | awk '{print $4}')
        p99=$(grep "99%" "$file" | awk '{print $2}')
        printf "│ %-10s │ %8s │ %9s ms │ %10s ms │\n" "c$concurrency" "$rps" "$time" "$p99"
    fi
done

echo "└────────────┴──────────┴─────────────┴──────────────┘"
echo ""

warn "Note: For best results, ensure rate_limit_per_second is set to 0"
echo ""