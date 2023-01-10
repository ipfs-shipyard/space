# Overview

This block-stream application provides the foundation for creating and exploring a new data transfer protocol designed for the IPFS-in-space scenario. The current implementation is pretty simple, it reads in the contents of a file, breaks up the contents into blocks, and transmits those blocks in UDP packets to a receiver. The blocks are serialized into binary data using the Parity SCALE format. The receiver listens for the stream of blocks, attempts to find the root block, and then waits until all links in the root are satisfied before assembling the file. This simple and naive approach to IPFS data transfer is intended to lay a foundation of point-to-point block streaming to be iterated on in future project milestones.

## Usage

First start the receiving/listening instance:

    $ cargo run -- receive /path/to/new/file 127.0.0.1:8080

This command will start an instance of the `block-streamer` which is listening at `127.0.0.1:8080` and will attempt to assemble the blocks it receives into a file located at `/path/to/new/file`.

Next start the transmitting instance:

    $ cargo run -- transmit /path/to/file 127.0.0.1:8080

This command will start an instance of `block-streamer` which will break up the file at `/path/to/file` into blocks, and then transmit those blocks in UDP packets to `127.0.0.1:8080`. Currently the blocks are sorted into random order prior to transmission in order to exercise the assembly functionality on the listening side.