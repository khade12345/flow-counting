# flow-counting

A reference implementation of the flow-counting algorithm for a hybrid pixel detector. 
This is a fork of the code by Kuttruff et al. 
https://github.com/m31k0r/flow-counting/actions/workflows/rust.yml/badge.svg

Features:
- Added reading of TPX3 files
- Added the option to output unclustered hits/events
- Parallelised file reading and clustering


Example use:
`cargo run -- --help`

time RUSTFLAGS="-C target-cpu=native" cargo run --release -- \
        --eps-time=50e-9 \
        --eps-pixel=5 \
        --cutoff=10 \
        --output-clusters output_clusters_kuttruff/output_tpx_clustering3.hdf5 \
        --n-threads=8 \
        --input data/2x4-hits-full-reorganised.hdf5 \
        --output-hits ./output_clusters_kuttruff/test_hits_no_sort.hdf5  \




This can run >30Mhits/s on an M1 Macbook Pro

## Implementation Detail
To minimize costly moves, we merged the output and buffer to one vector, and only compare with the *B* last elements in the output.

If you want to deploy this algorithm in a real-time toolchain it should be optimized to allow a fast enough memory throughput.

## Usage

Build it like any other Rust program with cargo and provide a suitable hdf5 file. The software looks for the keys:

key | description
----|------------
x   | x pixel of the hit
y   | y pixel of the hit
tot | time over threshold
toa | time of arrival
