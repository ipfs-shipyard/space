## Overview

This utility is a simple way to pack individual files into CAR archives and extract packed files from a CAR archive. It currently only supports one fs file per CAR archive.

## Usage

### Packing a file

    $ car-utility pack /path/to/input/file /path/to/archive.car

### Unpacking a file

    $ car-utility unpack /path/to/archive.car /path/to/output/file

## Dependencies

These system dependencies are required to build:
- Rust v1.63
- [Protobuf compiler](https://github.com/protocolbuffers/protobuf#protocol-compiler-installation): Download it from the [Protobuf Releases page](https://github.com/protocolbuffers/protobuf/releases)

## Cross-compiling for Raspberry Pi

### General Setup

Install `cross` tool for the cross-compiling environment:

    $ cargo install cross --git https://github.com/cross-rs/cross

Make sure `Docker` is installed and running.

### Building the app

The build command for the rasberry pi target is:

    $ cross build --target armv7-unknown-linux-gnueabihf

It is generally a good idea to run `cargo clean` between building for different targets, such as building for your local machine and then building for the raspi, otherwise the cross build may throw some weird glibc errors.

The built executable will be located at `target/armv7-unknown-linux-gnueabihf/[release|debug]/car-utility` and can now be transferred to the raspberry pi for usage.