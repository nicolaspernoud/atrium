###########################
# Stage 1 : Backend build #
###########################

# Versions
ARG RUST_VERSION
ARG FLUTTER_VERSION

# Set up an environnement to cross-compile the app for musl to create a statically-linked binary
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION} AS backend-builder
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
    "linux/amd64") echo x86_64-unknown-linux-gnu > /rust_target.txt ;; \
    "linux/arm64") echo aarch64-unknown-linux-gnu > /rust_target.txt ;; \
    "linux/arm/v7") echo armv7-unknown-linux-gnueabihf > /rust_target.txt ;; \
    "linux/arm/v6") echo arm-unknown-linux-musleabihf > /rust_target.txt ;; \
    *) exit 1 ;; \
    esac
RUN rustup target add $(cat /rust_target.txt)
RUN apt update && apt install -y binutils-arm-linux-gnueabihf clang cmake gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf libc6-dev-i386 libcap2-bin libclang-dev musl-dev musl-tools
RUN ln -s /usr/bin/arm-linux-gnueabihf-gcc /usr/bin/arm-linux-musleabihf-gcc
RUN ln -s /usr/bin/aarch64-linux-gnu-gcc /usr/bin/aarch64-linux-musl-gcc
RUN ln -s /usr/include/asm-generic /usr/include/asm

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

WORKDIR /build

COPY ./backend/.cargo ./.cargo
COPY ./backend/Cargo.toml ./
COPY ./backend/src ./src
COPY ./backend/tests ./tests

#RUN cargo test --release --target $(cat /rust_target.txt)
RUN cargo build --profile release_optimized --target $(cat /rust_target.txt)
RUN cp target/$(cat /rust_target.txt)/release_optimized/atrium .
RUN chown -f "${UID}":"${UID}" ./atrium
# Allow running on ports < 1000
RUN setcap cap_net_bind_service=+ep ./atrium

RUN mkdir -p /myapp/app
COPY ./backend/atrium.yaml /myapp/app
COPY ./backend/web/onlyoffice/ /myapp/app/web/onlyoffice/
COPY ./backend/web/oauth2/ /myapp/app/web/oauth2/
RUN chown -Rf "${UID}":"${UID}" /myapp

############################
# Stage 2 : Frontend build #
############################

FROM --platform=$BUILDPLATFORM ghcr.io/cirruslabs/flutter:${FLUTTER_VERSION} AS frontend-builder
WORKDIR /build
COPY ./frontend .
RUN flutter pub get
RUN flutter build web

#########################
# Stage 3 : Final image #
#########################

FROM --platform=linux/amd64 gcr.io/distroless/cc-debian12 AS base-amd64
FROM --platform=linux/arm64 gcr.io/distroless/cc-debian12 AS base-arm64
FROM --platform=linux/arm/v7 gcr.io/distroless/cc-debian12 AS base-armv7
FROM --platform=linux/arm/v6 scratch AS base-armv6

FROM base-${TARGETARCH}${TARGETVARIANT}

COPY --from=backend-builder /etc/passwd /etc/passwd
COPY --from=backend-builder /etc/group /etc/group

COPY --from=backend-builder /myapp /
WORKDIR /app
COPY --from=backend-builder /build/atrium ./
COPY --chown=appuser:appuser --from=frontend-builder /build/build/web/ /app/web/

USER appuser:appuser
ENTRYPOINT ["./atrium"]
