# Myceli Basic Setup and Usage Guide

This document covers the steps required to get a `myceli` instance up and running on both a raspbery-pi and local computer, and to begin transferring data between the two instances.

## Dependencies

These system dependencies are required to build:
- Rust v1.63
- [Protobuf compiler](https://github.com/protocolbuffers/protobuf#protocol-compiler-installation): Download it from the [Protobuf Releases page](https://github.com/protocolbuffers/protobuf/releases)
- Docker

## Prerequisites

This guide assumes you have already followed the steps in the [`Setup Local Environment Guide`](setup-local-environment.md), if you haven't then please work through that first.

Install `cross` tool for the cross-compiling environment:

    $ cargo install cross --git https://github.com/cross-rs/cross

Make sure `Docker` is installed and running.

## Build Myceli

The first step in using `myceli` to transfer data is building it for the raspberry-pi and local computer.

### Building for the raspberry-pi

Navigate to the `space/myceli` directory and run the following build command:

    $ CROSS_CONFIG=Cross.toml cross build --target armv7-unknown-linux-gnueabihf

This will kick off the cross-compiling process for the `myceli` project. After it has completed, you will find the finished binary at `space/target/armv7-unknown-linux-gnueabihf/debug/myceli`. This binary can now be transferred to the raspberry-pi for usage. A typical way to transfer this binary is with `scpp`, like so:

    $ scp target/armv7-unknown-linux-gnueabihf/debug/myceli pi@pi-address:/home/pi/

### Building for the local computer

Navigate to the `space/myceli` directory and run the following build command:

    $ cargo build

This will kick off the build process for the `myceli` binary. After it has completed, you will find the finished binary at `space/target/debug/myceli`. 

## Running Myceli

After `myceli` has been built for the appropriate environments it needs to be run with the correct configuration info.

### Running on the raspberry-pi

Use ssh to access the raspberry-pi and navigate to the `/home/pi` directory. Start a `myceli` instance with the following command:

    $ ./myceli

This command assumes that the pi currently has a radio service running which is downlinking to the address `127.0.0.1:8080`, as specified in the local environment setup guide. 

A log message should appear indicating that `myceli` is up and listening:

    $ INFO myceli::listener: Listening for messages on 127.0.0.1:8080

### Running on the local computer

Navigate to the `space/myceli` directory and run the following command:

    $ cargo run

This command assumes that the local computer has a radio service running which is downlinking to the address `127.0.0.1:8080`, as specified in the local environment setup guide.

A log message should appear indicating that `myceli` is up and listening:

    $ INFO myceli::listener: Listening for messages on 127.0.0.1:8080

## Configuring Myceli

`myceli` has a few configuration options which ship with default values, or can be tuned to fit system requirements.

Current configuration values and defaults are:
- `listen_address` - The network address `myceli` will listen on for incoming messages. Defaults to `127.0.0.1:8080`.
- `retry_timeout_duration` - Timeout before `myceli` will retry a dag transfer, measured in milliseconds. The default value is 120_00 or two minutes.
- `storage_path` - Directory path for `myceli` to use for storage. If this directory does not exist it will be created. Defaults to `storage/` in the process working directory.

These configuration values can be set via a TOML config file which is passed as an argument when running `myceli`.

Here is an example configuration file:

    listen_address="127.0.0.1:9011"
    retry_timeout_duration=360_000
    storage_path="myceli_storage"

If this configuration is saved to "myceli.toml", then we would run `myceli myceli.toml` to use the config file.

## Interacting with Myceli

Now that `myceli` has been built and is running on both the raspberry-pi and local computer, commands may be sent to the instances to control them.

Navigate to `space/app-api-cli` and run `cargo build` to build the tool we'll use for interacting with `myceli`. After the `app-api-cli` is built we'll walk through some basic commands.

### Importing a file

One of the fundamental actions `myceli` can take is importing a file into it's internal IPFS store. Navigate to `space/app-api-cli` and run the following command to import a local file:

    $ cargo run -- -l 127.0.0.1:8080 import-file Cargo.toml

This will send the `ImportFile` command to the local `myceli` instance listening at `127.0.0.1:8080` with the local `Cargo.toml` as the file to import. In this case we'll use the `-l` flag to listen for a response, as `myceli` will respond with the root CID if the file is correctly imported. Here is what the output may look like for a successful file import:

    Transmitting: {"ApplicationAPI":{"ImportFile":{"path":"Cargo.toml"}}}
    ApplicationAPI(FileImported { path: "Cargo.toml", cid: "bafybeicwxyav7jde73wb5svahp53qi5okq2p4bguyflfw6hsbmwbbl4bw4" })

### Transmitting a dag

Once a file has been imported, and the root CID is known, it is possible to ask the `myceli` instance holding that file in storage to transmit it to another `myceli` instance. In this case we'll transmit from the local computer to the raspberry-pi.

On the local computer, in the `app-api-cli` directory, run the following command:

    $ cargo run -- 127.0.0.1:8080 transmit-dag [root-cid-here] 127.0.0.1:8081 5

This will send the `TransmitDag` command to the `myceli` instance listening on `127.0.0.1:8080`, which will ask it to transmit the blocks associated with the specified root CID to `127.0.0.1:8081` with `5` specified as the number of retries. After sending this command you should see several `Transmitting block ...` messages from the local computer's `myceli`, and several `Received block ...` messages from the raspberry-pi's `myceli`.

### Validating a dag

After a dag has been transmitted, it must be verified that it is complete and valid at the destination. 

To verify the status of the dag on the raspberry-pi, run the following app-api-cli command:

    $ cargo run -- 127.0.0.1:8081 validate-dag [root-cid-here]

This will send the `ValidateDag` command to the radio listening at `127.0.0.1:8081`, which will then send it to the `myceli` instance on the raspberry-pi. In this case, the response will appear in the logs for the local computer's `myceli` instance. Check over there for `ValidateDagResponse` and the `result` string. If it says `Dag is valid`, then we know the transfer was complete and valid.


### Exporting a dag

Once a dag has been transmitted and validated, it can be exported as a file on the receiving system.

To export the received dag on the raspberry-pi, run the following app-api-cli command:

    $ cargo run -- 127.0.0.1:8081 export-dag [root-cid-here] [/file/system/path]

This will send the `ExportDag` command to the radio listening at `127.0.0.1:8081`, which will send it to the `myceli` instance on the rasberry-pi. This command includes the specified root cid and path to export to. After the command has been received and executed, you should find a file at the specified path containing the dag data.

