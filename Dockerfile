###########################
# Stage 1 : Backend build #
###########################

# Set up an environnement to cross-compile the app for musl to create a statically-linked binary
FROM --platform=$BUILDPLATFORM rust:1.63 AS backend-builder
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") echo x86_64-unknown-linux-musl > /rust_target.txt ;; \
      "linux/arm64") echo aarch64-unknown-linux-musl > /rust_target.txt ;; \
      "linux/arm/v7") echo armv7-unknown-linux-musleabihf > /rust_target.txt ;; \
      "linux/arm/v6") echo arm-unknown-linux-musleabihf > /rust_target.txt ;; \
      *) exit 1 ;; \
    esac
RUN rustup target add $(cat /rust_target.txt)
RUN apt update && apt install -y musl-tools musl-dev binutils-arm-linux-gnueabihf gcc-arm-linux-gnueabihf gcc-aarch64-linux-gnu libcap2-bin
RUN ln -s /usr/bin/arm-linux-gnueabihf-gcc /usr/bin/arm-linux-musleabihf-gcc
RUN ln -s /usr/bin/aarch64-linux-gnu-gcc /usr/bin/aarch64-linux-musl-gcc
RUN update-ca-certificates

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
RUN cargo build --release --target $(cat /rust_target.txt)
RUN cp target/$(cat /rust_target.txt)/release/atrium .
# Allow running on ports < 1000
RUN setcap cap_net_bind_service=+ep ./atrium

RUN mkdir -p /myapp/app
COPY ./backend/atrium.yaml /myapp/app
RUN chown -Rf "${UID}":"${UID}" /myapp

############################
# Stage 2 : Frontend build #
############################

FROM --platform=$BUILDPLATFORM cirrusci/flutter:3.3.0 as frontend-builder
WORKDIR /build
COPY ./frontend .
RUN flutter pub get
RUN flutter build web --csp
RUN sed -i "s/serviceWorkerVersion = null/serviceWorkerVersion = '$(shuf -i 1000000000-9999999999 -n 1)'/g" ./build/web/init.js

#########################
# Stage 3 : Final image #
#########################

FROM scratch

COPY --from=backend-builder /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=backend-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=backend-builder /etc/passwd /etc/passwd
COPY --from=backend-builder /etc/group /etc/group

COPY --from=backend-builder /myapp /
WORKDIR /app
COPY --from=backend-builder /build/atrium ./
COPY --from=frontend-builder /build/build/web/ /app/web/
COPY ./backend/src/web/onlyoffice/ /app/web/onlyoffice/

USER appuser:appuser
ENTRYPOINT ["./atrium"]
