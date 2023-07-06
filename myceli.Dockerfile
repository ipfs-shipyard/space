# Base build stage
FROM rust:1.67 as builder
# Install protobuf compiler
RUN curl -Lo protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v22.2/protoc-22.2-linux-x86_64.zip"
RUN unzip protoc.zip -d protoc/
RUN cp -a protoc/* /usr/local

# Copy over and build myceli
COPY . .
RUN cargo build --bin myceli --features big
RUN cp ./target/debug/myceli /usr/bin/myceli

# Extras stage
FROM debian:bullseye-slim
LABEL org.opencontainers.image.source="https://github.com/ipfs-shipyard/space"
COPY --from=builder /usr/bin/myceli /usr/bin/myceli
COPY --from=builder Cargo.toml /usr/local/Cargo.toml
ENTRYPOINT myceli $CONFIG_PATH
