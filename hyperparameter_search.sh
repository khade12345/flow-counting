#!/bin/bash

# filepath: ./hyperparameter_search.sh

# Create hyperparameters directory if it doesn't exist
mkdir -p ./hyperparameters

# Output file
OUTPUT_FILE="./hyperparameters/hparam_search.txt"

# Clear previous results
> "$OUTPUT_FILE"

# Array of sort window values to test
SORT_WINDOWS=(1 10 50 100 500 1000 5000 10000 50000 10000000000)

# Fixed parameters
EPS_TIME="50e-9"
EPS_PIXEL="5"
CUTOFF="10"
N_THREADS="8"
FILE="./data_tpx/raw_events_1kx1k_10us_30mhits.tpx3"
OUTPUT_DIR="./hyperparameters"

echo "Starting hyperparameter search..."
echo "================================" | tee -a "$OUTPUT_FILE"

for SORT_WINDOW in "${SORT_WINDOWS[@]}"; do
    echo ""
    echo "Testing sort_window=$SORT_WINDOW"
    echo "Testing sort_window=$SORT_WINDOW" >> "$OUTPUT_FILE"
    
    OUTPUT_EVENT="$OUTPUT_DIR/events_sw_${SORT_WINDOW}.hdf5"
    TEST_OUTPUT="$OUTPUT_DIR/clusters_sw_${SORT_WINDOW}.hdf5"
    
    # Run the cargo command and capture output
    OUTPUT=$(time cargo run --release -- \
        --eps-time="$EPS_TIME" \
        --eps-pixel="$EPS_PIXEL" \
        --cutoff="$CUTOFF" \
        --output "$TEST_OUTPUT" \
        --n-threads="$N_THREADS" \
        --file "$FILE" \
        --output-event "$OUTPUT_EVENT" \
        --sort-window="$SORT_WINDOW" 2>&1)
    
    # Extract the "nhits" line
    NHITS_LINE=$(echo "$OUTPUT" | grep "nhits = ")
    if [ -n "$NHITS_LINE" ]; then
        echo "$NHITS_LINE" >> "$OUTPUT_FILE"
    else
        echo "ERROR: Could not find 'nhits' in output" >> "$OUTPUT_FILE"
    fi
    
    # Extract the "clusters length" line
    CLUSTERS_LINE=$(echo "$OUTPUT" | grep "clusters length")
    if [ -n "$CLUSTERS_LINE" ]; then
        echo "$CLUSTERS_LINE" >> "$OUTPUT_FILE"
    else
        echo "ERROR: Could not find 'clusters length' in output" >> "$OUTPUT_FILE"
    fi
    
    # Extract timing information (real time from 'time' command)
    TIME_LINE=$(echo "$OUTPUT" | grep "real")
    if [ -n "$TIME_LINE" ]; then
        echo "  Time: $TIME_LINE" >> "$OUTPUT_FILE"
    fi
    
    echo "  Completed" >> "$OUTPUT_FILE"
    echo ""
done

echo ""
echo "================================" | tee -a "$OUTPUT_FILE"
echo "Hyperparameter search complete!"
echo "Results saved to: $OUTPUT_FILE"
cat "$OUTPUT_FILE"