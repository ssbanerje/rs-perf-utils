# Rust Utilities for Interfacing with Linux Perf

## Build
To build the project on Debian based machines:
1. Install dependencies
    ```
    sudo apt install llvm-dev libclang-dev clang
    ```
2. Install `perf` and configure
    ```
    sudo apt install linux-tools-common linux-tools-generic linux-tools-`uname -r`
    sudo sysctl -w kernel.perf_event_paranoid=-1
    ```  
2. Build this project
    ```
    cargo build -release
    ```

## Run Tests
To run tests execute:
```
PMU_EVENTS=<perfmon folder> cargo test
```

For Intel processors the performance counter metadata can be downloaded from [01.org](`https://download.01.org/perfmon/`).
```
wget -r --no-parent https://download.01.org/perfmon/
find download.01.org -name "index.html*" -delete 
```

Alternatively the [Linux kernel source](https://github.com/torvalds/linux/tree/master/tools/perf/pmu-events/arch) hosts similar metadata for several ISAs.

## Examples / Tools
To dump event strings for the perf command line tool from the JSON metadata.
```
cargo run --example dump_perf_strings <perfmon folder>
```
