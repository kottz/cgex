#!/bin/bash

set -e

# Expected file counts (placeholders)
EXPECTED_MJOLNER=420
EXPECTED_DJUPET=420
EXPECTED_MULLEBIL=420

# Function to run a single test
run_test() {
    local game=$1
    local options=$2
    local temp_output="/tmp/cgex_test_${game}_${RANDOM}"
    
    echo "Testing $game with options: $options"
    
    docker run --rm \
        -v "./disc_contents_${game}:/input" \
        -v "${temp_output}:/output" \
        -e HOST_UID=$(id -u) \
        -e HOST_GID=$(id -g) \
        $options \
        cgex
    
    local file_count=$(find "${temp_output}" -type f \( -name "*.bmp" -o -name "*.webp" -o -name "*.png" \) | wc -l)
    
    echo "File count for $game: $file_count"
    
    local expected_var="EXPECTED_${game^^}"
    if [ "$file_count" -eq "${!expected_var}" ]; then
        echo "$game test passed!"
    else
        echo "$game test failed. Expected ${!expected_var} files, got $file_count"
    fi
    
    echo "Cleaning up ${temp_output}"
    rm -rf "${temp_output}"
}

# Build the Docker image
echo "Building Docker image..."
docker build -t cgex .

# Run tests in parallel
echo "Running tests..."
run_test mjolner "" &
run_test djupet "" &
run_test mullebil "" &
run_test mjolner "-e NO_COMPRESSION=true -e NO_UPSCALE=true" &
run_test djupet "-e NO_COMPRESSION=true -e NO_UPSCALE=true" &
run_test mullebil "-e NO_COMPRESSION=true -e NO_UPSCALE=true" &

# Wait for all background jobs to finish
wait

echo "All tests completed."
