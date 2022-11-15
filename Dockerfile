FROM ghcr.io/cross-rs/armv7-unknown-linux-gnueabihf:latest

RUN apt-get update && \
    apt-get install -y unzip

RUN curl -Lo protoc.zip "https://github.com/protocolbuffers/protobuf/releases/latest/download/protoc-21.9-linux-x86_64.zip"
RUN unzip -q protoc.zip -d /usr/local
RUN chmod a+x /usr/local/bin/protoc