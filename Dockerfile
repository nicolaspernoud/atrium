# syntax=docker/dockerfile:1

# Default versions (can be overridden with --build-arg)
ARG RUST_VERSION=1.94
ARG FLUTTER_VERSION=3.41.5

# --- Frontend Builder ---
# Use the Flutter image to build the web assets
FROM --platform=$BUILDPLATFORM ghcr.io/cirruslabs/flutter:${FLUTTER_VERSION} AS frontend-builder
WORKDIR /src
# Build the Flutter web app
COPY --exclude=.dart_tool frontend .
RUN flutter build web --release
# Add the static web assets
COPY --exclude=index.html backend/web/. build/web/
# Gzip assets for the backend to serve gzipped files
RUN find build/web -type f ! -name "*.gz" ! -name "*.tmpl" -exec gzip -9 {} +

# --- Backend Builder ---
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS backend-builder
ARG TARGETPLATFORM

# Install cross-compilation dependencies
RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    gcc-aarch64-linux-gnu \
    gcc-arm-linux-gnueabihf \
    libc6-dev-i386 \
    libcap2-bin \
    libclang-dev \
    musl-dev \
    musl-tools \
    wget \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN ln -s /usr/include/asm-generic /usr/include/asm

# Handle armv6 musl toolchain if needed
RUN if [ "$TARGETPLATFORM" = "linux/arm/v6" ]; then \
    wget https://github.com/cross-tools/musl-cross/releases/download/20250929/arm-unknown-linux-musleabihf.tar.xz -O - | tar -xJf - -C /opt; \
    fi
ENV PATH="/opt/arm-unknown-linux-musleabihf/bin:$PATH"

WORKDIR /app

# Determine the rust target
RUN case "$TARGETPLATFORM" in \
    "linux/amd64") RUST_TARGET="x86_64-unknown-linux-gnu" ;; \
    "linux/arm64") RUST_TARGET="aarch64-unknown-linux-gnu" ;; \
    "linux/arm/v7") RUST_TARGET="armv7-unknown-linux-gnueabihf" ;; \
    "linux/arm/v6") RUST_TARGET="arm-unknown-linux-musleabihf" ;; \
    *) RUST_TARGET="x86_64-unknown-linux-gnu" ;; \
    esac; \
    echo "$RUST_TARGET" > /rust_target_name && \
    rustup target add "$RUST_TARGET"

# Copy frontend assets from frontend-builder
# We copy them to backend/dist because backend/src/web.rs uses #[folder = "dist/"]
COPY --from=frontend-builder /src/build/web ./backend/dist

# Copy backend source
COPY backend/Cargo.toml ./backend/
COPY backend/.cargo ./backend/.cargo
COPY backend/src ./backend/src

WORKDIR /app/backend
RUN RUST_TARGET=$(cat /rust_target_name) && \
    cargo build --profile release_optimized --target "$RUST_TARGET" && \
    cp target/"$RUST_TARGET"/release_optimized/atrium /atrium

# --- Binary Exporter ---
# This stage is used to extract the binary locally
FROM scratch AS binary-exporter
ARG TARGETARCH
ARG TARGETVARIANT
COPY --from=backend-builder /atrium /atrium-${TARGETARCH}${TARGETVARIANT}

# --- Final Prep ---
# Prepare the binary and environment in a temporary stage
FROM --platform=$BUILDPLATFORM debian:trixie-slim AS prep
ARG TARGETPLATFORM
RUN apt-get update && apt-get install -y adduser libcap2-bin && rm -rf /var/lib/apt/lists/*
# Create appuser
ENV USER=appuser
ENV UID=1000
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

RUN mkdir -p /myapp/app
COPY --from=backend-builder /atrium /myapp/app/atrium
RUN chown -Rf "${UID}":"${UID}" /myapp/app/
# Allow running on ports < 1000
RUN setcap cap_net_bind_service=+ep /myapp/app/atrium

# --- Final Image ---
FROM --platform=linux/amd64 gcr.io/distroless/cc-debian13 AS base-amd64
FROM --platform=linux/arm64 gcr.io/distroless/cc-debian13 AS base-arm64
FROM --platform=linux/arm/v7 gcr.io/distroless/cc-debian13 AS base-armv7
FROM --platform=linux/arm/v6 scratch AS base-armv6

ARG TARGETARCH
ARG TARGETVARIANT
FROM base-${TARGETARCH}${TARGETVARIANT}
COPY --from=prep /etc/passwd /etc/passwd
COPY --from=prep /etc/group /etc/group
COPY --from=prep --chown=appuser:appuser /myapp /

WORKDIR /app
USER appuser:appuser
ENTRYPOINT ["./atrium"]