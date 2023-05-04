FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:0.2.5

RUN dpkg --add-architecture arm64 && apt-get update && \
    apt-get install -y unzip libssl-dev:arm64

RUN curl -Lo protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v22.2/protoc-22.2-linux-x86_64.zip"
RUN unzip -q protoc.zip -d /usr/local
RUN chmod a+x /usr/local/bin/protoc
ENV PROTOC=/usr/local/bin/protoc