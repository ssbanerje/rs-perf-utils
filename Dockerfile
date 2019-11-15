FROM ubuntu:rolling
MAINTAINER Subho S Banerjee <ssbaner2@illinois.edu>

# Setup dependencies
RUN apt update \
    && apt install -y build-essential llvm-dev libclang-dev clang linux-tools-common linux-tools-generic \
        linux-tools-`uname -r` linux-headers-`uname -r` curl wget \
    && rm -rf /var/lib/apt/lists*

# Setup Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y \
    && echo "source $HOME/.cargo/env" >> $HOME/.bashrc

# Get perfmon events
RUN wget -r --no-parent https://download.01.org/perfmon/ | true \
    && find download.01.org -name "index.html*" -delete \
    && mkdir -p /src \
    && mv download.01.org/perfmon /src/perfmon \
    && rm -rf download.01.org
ENV PMU_EVENTS=/src/perfmon
