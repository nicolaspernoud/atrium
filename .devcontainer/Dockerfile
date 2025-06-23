FROM ghcr.io/cirruslabs/flutter:stable

# Install dependencies
RUN apt update && apt install -y \
    clang \
    cmake \
    libssl-dev \
    pkg-config

# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc