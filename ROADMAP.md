# Roadmap

## Overview

This document sketches out a roadmap of proof of concept and minimum viable product milestones on the road of content addressable data in space 🚀. 

## Milestone 1

Milestone one focuses on basic system and hardware bring up.

- [x] [**PoC - CAR utility**](https://github.com/ipfs-shipyard/space/issues/2) - Create a command line utility based on [Iroh](https://github.com/n0-computer/iroh) components which can pack any file into a CAR file and unpack/reconstruct the contents of a CAR file.
- [x] [**PoC - Generate & transmit CAR**](https://github.com/ipfs-shipyard/space/issues/3) - Run the CAR utility on the raspberry pi to generate a CAR file from a known payload. Transmit using an existing file transfer protocol over ethernet connection to laptop and reassemble original payload using CAR utility.
- [x] [**PoC - Bring up radio communications**](https://github.com/ipfs-shipyard/space/issues/4) - Create radio drivers on raspberry pi and desktop ends which provide a programmable interface into the radio. Create a communications service on both radio ends which provides a generic way to send and receive data over the radio. Demonstrate sending ping back and forth over radio between raspberry pi and desktop.
- [x] [**MVP v0.1 - Generate & transmit CAR over radio**](https://github.com/ipfs-shipyard/space/issues/5) - Generate CAR file using utility on raspberry pi. Use known file transfer protocol to transmit over radio interface to ground station. Ground station should receive, reassemble, and verify payload. Radio connection should be persistent and reliable.

## Milestone 2

Milestone two focuses on creating a bare minimum method of transferring a file using IPFS components.

- [x] **PoC - Establish one way block-ship stream** - Create a rough implementation of a transmitter and receiver of a stream of DAG blocks. This will provide the two ends necessary to develop and iterate on a block-ship protocol in the future. This implementation will be based on iroh, one way, no retransmissions or feedback, but it should have a tunable packet size. The file under transmission will be streamed into DAG blocks once per transmission and the blocks will not be persisted.
- [x] **MVP v0.2 - Generate DAG, transmit & receive over Radio** - The block-ship pieces from previous proof-of-concept will be deployed on the raspberry pi and computer and used to demonstrate sending a one-way stream of DAG blocks over the radio link. The transmitter will be tuned as appropriate to work under the dev environment's transmission limitations and slowed down to ensure successful transmission in one go. 

## Milestone 3

Milestone three focuses on exploring a basic application API.

- [ ] [**PoC - Prototype application API**](https://github.com/ipfs-shipyard/space/pull/16) - Take a first pass at the application API functionality required to implement basic IPFS-in-space scenarios, such as requesting a CID from space to ground. Implement this API using JSON over UDP to get an easy sense of API usage. Implement a basic cli utility for sending API commands. Only implement actually responding to api messages to transmit and receive.
- [ ] **PoC - Investigate binary messaging format** - The initial application API is implemented in JSON, but that format will not be suitable for real-world usage. Investigate other formats such as [cbor](https://cbor.io/) and decide which to use for all message formatting, including this API.
- [ ] **PoC - Prototype interleaving application API and data transfer protocol** - In the initial application API the API messages and data transfer protocol messages are handled in independent messaging "sessions". This system will need the ability to handle either type of these messages at the same time on the same port. Implement a higher level message type which can support either the application API or data transfer protocol.
- [ ] **MVP 0.3 - Demonstrate application API over radio** - Demonstrate both space & ground instances of IPFS which can receive control messages to *transmit* and *receive* files. Use these control messages to command the ground instance to transmit a file to the space instance, all using the same radio link.

## Milestone 4

Milestone four focuses on implementing basic file/CID handling APIs.

- [ ] **PoC - Implement `Import File` and `Export CID` APIs** - Decide on a basic IPFS storage layer, and then implement the APIs for importing & exporting a file to/from that storage layer.
- [ ] **PoC - Implement `Available CIDs` and `Request CID` APIs** - Building on the IPFS storage layer, and APIs for importing exporting. This exposes available CIDs via API, and allows requesting the transfer of a specific CID if available.
- [ ] **MVP 0.4 - Demonstrate a file import and transfer request**

## Milestone 5

Milestone five focuses on CID and block validation

- [ ] **PoC - Implement block-level validation** - Implement validation on a per-block basis as they are received and assembled.
- [ ] **PoC - Implement `Is CID Complete` and `Validate CID` APIs** - Implement an API to determine if a CID is present with all of it's children blocks, and an API to validate that CID all the way down.
- [ ] **MVP 0.5 - Demonstrate CID complete/validate APIs after transfer**

## Milestone 6

Milestone six focuses on APIs for gathering pass/connectedness info, and incorporating that info into the transfer process.

- [ ] **PoC - Implement `Is Connected` and `Next Pass Info` APIs** - Implement APIs to be used by external systems to indicate if the system is connected and to give information about the next pass.
- [ ] **PoC - Modify data transfer protocol to account for pass/connectedness info** - Update the data transfer protocol to track pass/connectedness information and determine when it should be transmitting data based on known pass information.
- [ ] **MVP 0.6 - Simulate a "pass" using implemented APIs and demonstrate how data transfer responds**

## Milestone 7

Milestone 7 focuses on implementing minimum retry methods for reliability.

- [ ] **PoC - Define and implement block-ship re-transmissions** - The initial implementation of the block-ship protocol is a very simple one-way stream of DAG blocks. Define a mechanism for detecting missing blocks, requesting transmission of specific blocks, and responding to those requests. Implement in block-ship transmitter and receiver.
- [ ] **PoC - Implement baseline metrics** - Implement a baseline set of metrics, perhaps [this list](https://github.com/n0-computer/test-plans/tree/main/movethebytes/data-transfer#metrics) suggested by move the bytes, and build into logging of block-ship transmitter and receiver.
- [ ] **MVP v0.7 - Demonstrate block-ship retransmit over radio** - Establish a block-ship session over the dev radio link at a speed which ensures minor packet loss and tune the re-transmission mechanism to overcome the packet loss.

## Future Milestones

- **PoC - Explore various satellite profiles** - Satellites operate across a variety of scenarios (earth observing, communications, navigation, weather, etc), each of which may require a different operational profile. Research these common scenarios, and their associated operational profiles, and create simulation scenarios to test against IPFS software.
- **Poc - Explore multi-block specification techniques** - Explore how to specify multiple blocks for request/transmission, such as by CID, sub-graph, bloom filter, etc.

## Future ideas

- Utilizing [testground](https://docs.testground.ai/) and [containernet](https://containernet.github.io/) for larger scale simulations
- Explore using the [Space Packet Protocol](https://egit.irs.uni-stuttgart.de/rust/spacepackets) or [AX.25](https://github.com/thombles/ax25-rs) for payload format
- Keep an eye on the output of the [Move the Bytes WG](https://www.notion.so/Move-the-Bytes-WG-2dc2194a004f4b72a9706ad5a150081d) and see if we can steal/borrow their protocol ideas