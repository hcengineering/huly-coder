# Build stage
FROM --platform=$BUILDPLATFORM rust:1.86 AS builder
ARG TARGETPLATFORM
WORKDIR /usr/src/huly-coder
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:12-slim

RUN apt-get update && apt-get install -y ca-certificates
ENV SHELL=/bin/bash

LABEL org.opencontainers.image.source="https://github.com/hcengineering/huly-coder"
COPY --from=builder /usr/src/huly-coder/target/release/huly-coder /usr/local/bin/huly-coder
COPY --from=builder /usr/src/huly-coder/huly-coder.yaml huly-coder.yaml
ENTRYPOINT ["/usr/local/bin/huly-coder"]