# Overview

The myceli application acts as the "node" in this IPFS-for-space project. The current design allows a myceli to act as a node either on a spacecraft or in a ground station. While myceli is running it can receive and respond to any API or data protocol messaging.

## Usage

Start an instance:

    $ cargo run -- 127.0.0.1:8080

This command will start a `myceli` instance which is listening at `127.0.0.1:8080` and will respond to any valid messages received on that address.

Next, send a command. The `app-api-cli` utility is a CLI tool used to generate and send messages to `myceli` instances. For example, we can ask the running instance which blocks it currently has available:

    $ cd app-api-cli && cargo run -- -l 127.0.0.1:8080 request-available-blocks

This will send a `RequestAvailableBlocks` message to the instance listening at `127.0.0.1:8080` and display the response when it is received.