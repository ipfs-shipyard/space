FROM ghcr.io/cross-rs/x86_64-unknown-linux-gnu:latest

RUN apt-get update && \
    apt-get install -y unzip libssl-dev

RUN curl -Lo protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v22.2/protoc-22.2-linux-x86_64.zip"
RUN unzip -q protoc.zip -d /usr/local
RUN chmod a+x /usr/local/bin/protoc
ENV PROTOC=/usr/local/bin/protoc