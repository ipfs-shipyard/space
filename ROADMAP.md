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

- [x] [**PoC - Prototype application API**](https://github.com/ipfs-shipyard/space/pull/16) - Take a first pass at the application API functionality required to implement basic IPFS-in-space scenarios, such as requesting a CID from space to ground. Implement this API using JSON over UDP to get an easy sense of API usage. Implement a basic cli utility for sending API commands. Only implement actually responding to api messages to transmit and receive.
- [x] **PoC - Investigate binary messaging format** - The initial application API is implemented in JSON, but that format will not be suitable for real-world usage. Investigate other formats such as [cbor](https://cbor.io/) and decide which to use for all message formatting, including this API. _The result of this work was deciding to stick with the SCALE codec for all messaging_.
- [x] [**PoC - Prototype interleaving application API and data transfer protocol**](https://github.com/ipfs-shipyard/space/pull/20) - In the initial application API the API messages and data transfer protocol messages are handled in independent messaging "sessions". This system will need the ability to handle either type of these messages at the same time on the same port. Implement a higher level message type which can support either the application API or data transfer protocol.
- [x] [**MVP 0.3 - Demonstrate application API**](https://www.loom.com/share/2c56c6d4297949f4929c84f4112e4eef) - Demonstrate both space & ground instances of IPFS which can receive control messages to *transmit* and *receive* files. Use these control messages to command the ground instance to transmit a file to the space instance.

## Milestone 4

Milestone four focuses on implementing basic file/CID handling APIs.

- [x] [**PoC - Implement `Import File`, `Export Dag`, and `Transmit Dag` APIs**](https://github.com/ipfs-shipyard/space/pull/21) - Decide on a basic IPFS storage layer, and then implement the APIs for importing & exporting a file to/from that storage layer, as well as transmitting a DAG, and exposing which blocks are available for transmission.
- [x] [**PoC - Implement chunking across all messages**](https://github.com/ipfs-shipyard/space/pull/24) - Some of the APIs introduced in the previous PoC result in messages which break the 60 byte MTU. Because of this, a general chunking method will need to be investigated and implemented across all messages in the system.
- [x] [**MVP 0.4 - Demonstrate a file import and transfer request**](https://www.loom.com/share/ad5c5509ac6b4adb8734dcf54cf6b3cf)

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

## Future Epics

- **Productionize Software** - All current milestones have been building out MVP-grade software, which will need to be made production grade. The software should be run through a profiler on several scenarios to check for excess memory usage. Memory allocation should be analyzed and optimized. All key data transfer paths should be instrumented to collect metrics and measure performance. APIs and communications interfaces need to be examined for vulnerabilities. Tests at scale should be run to determine performance limits inside of a variety of realistic simulated environments.

- **Optimize Data Transfer Protocol** - Explore more sophisticated methods for exchanging data. Investigate chunking methods tuned to specific data types to increase the chance of deduplicating data or allow for meaningful partial transfers. Explore how to specify multiple blocks for request/transmission, such as by CID, sub-graph, bloom filter, etc. Intelligently plan which data to transfer based on passes within constellations.

- **Multi-Radio Support** - Extend the communication interface to support transmitting and receiving from multiple sources. Build out logic for routing specific message types to specific radio/comms interfaces, such as one interface for space-to-space, and one interface for space-to-ground. This may be handled transparently by some systems, and on others it may require more manual attention. This could also include support for one-way communications interfaces, such as beacon radios (transmit only).

- **Satellite Constellation Support** - Extend the point-to-point functionality of space-to-ground communications to also support peer-to-peer communications within a satellite constellation. Design and implement the DHT equivalent for tracking which satellites in the constellation have which blocks. Create a discovery & transfer mechanism which can be adapted for different types inter-satellite-links and what information about peers is/isn't available.

- **Integrate Space IPFS with other IPFS** - Creating an interoperability layer between the space-to-ground IPFS network and other IPFS networks. This may include extending the ground IPFS node to present as a gateway or relay to other IPFS networks. The idea here is to support easy data exchange between more standard public/private IPFS networks and the IPFS data passed between space & ground. This could be used as a backhaul between ground stations or a method for publicly exposing extra-terrestrial data.

- **Groundstation data coordination** - An important aspect of using IPFS in conjunction with ground station networks is the coordination, exchange, and assembly of data transmitted by spacecraft. This could be accomplished by standing up an IPFS network for this purpose, integrating with a standard public/private IPFS network, by using a similar coordination method as the spacecraft constellation, or by a different means.

- **SDK the Project** - Generalize, package, and document the project to make it easily accessible and usable in third party missions. Define and create a rich API via static library for integration into systems built in other languages. Harden CLI tool for API commands and expand functionality. Potentially generalize the UDP interface to use the [Space Packet Protocol](https://egit.irs.uni-stuttgart.de/rust/spacepackets) or [AX.25](https://github.com/thombles/ax25-rs) formats to allow for more direct link integration.

- **Research broader satellite profiles** - Satellites operate across a variety of scenarios (earth observing, communications, navigation, weather, etc), each of which may require a different operational profile. Research these common scenarios, and their associated operational profiles, and create simulation scenarios to test against IPFS software. Create optimizations for constellation setup and data transfer protocol based on operational profile needs.

- **Simulations and CI** - Build out a simulation environment using [testground](https://docs.testground.ai/) or [containernet](https://containernet.github.io/). Build out tests for larger scale scenarios across multiple satellites and ground stations. Build out simple scenarios which can be run regularly in CI.

- **Static/human names** - Implement a a system like IPNS or DNSLink to expose static or human-readable names for singular assets or lists of assets.