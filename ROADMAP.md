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

Milestone five focuses on DAG and block validation

- [x] [**PoC - Implement block-level validation**](https://github.com/ipfs-shipyard/space/pull/33) - Implement validation on a per-block basis as they are received and assembled.
- [x] [**PoC - Implement `ValidateDag` APIs**](https://github.com/ipfs-shipyard/space/pull/34) - Implement an API to validate a DAG.
- [x] [**MVP 0.5 - Demonstrate DAG complete/validate APIs after transfer**](https://youtu.be/gyed10oWHk0) - Demonstrate the `ValidateDag` API correctly detecting when a DAG has and hasn't been successfully transmitted over radio.

## Milestone 6

Milestone six focuses on implementing minimum retry methods for reliability.

- [x] [**PoC - Define and implement block re-transmissions**](https://github.com/ipfs-shipyard/space/pull/40) - The initial implementation of the block transfer protocol is a very simple one-way stream of DAG blocks. Define a mechanism for detecting missing blocks, requesting transmission of specific blocks, and responding to those requests. Implement in myceli both transmissions and responses.
- [ ] **MVP v0.6 - Demonstrate block retransmit over radio** - Establish a myceli session over the dev radio link at a speed which ensures minor packet loss and tune the re-transmission mechanism to overcome the packet loss.

## Milestone 7

Milestone seven focuses beginning integrating myceli with kubo.

- [ ] **PoC - Implement initial myceli to kubo bridge** - Utilize the kubo rpc api to implement a bridge process which syncs blocks received by a myceli node into a kubo node.
- [ ] **MVP 0.7 - Demonstrate first myceli to kubo integration** - Demonstrate downlinking a dag over the radio to a myceli ground node, syncing that dag to a kubo node, and then viewing the representative file hosted on IPFS.
## Milestone 8

Milestone seven focuses on APIs for gathering pass/connectedness info, and incorporating that info into the transfer process.

- [ ] **PoC - Implement `Is Connected` and `Next Pass Info` APIs** - Implement APIs to be used by external systems to indicate if the system is connected and to give information about the next pass.
- [ ] **PoC - Modify data transfer protocol to account for pass/connectedness info** - Update the data transfer protocol to track pass/connectedness information and determine when it should be transmitting data based on known pass information.
- [ ] **PoC - Implement baseline metrics** - Implement a baseline set of metrics, perhaps [this list](https://github.com/n0-computer/test-plans/tree/main/movethebytes/data-transfer#metrics) suggested by move the bytes, and build into logging of block-ship transmitter and receiver.
- [ ] **MVP 0.8 - Simulate a "pass" using implemented APIs and demonstrate how data transfer responds**

## Future Epics

These are potential future epics or topics of work which may be incorporated into the roadmap.

### Productionize Software

All current milestones have been building out MVP-grade software, which will need to be made production grade. 

- All known mission related scenarios should be tested to flush out missing functionality and edge cases.
- The software should be run through a profiler on several scenarios to check for excess memory usage. 
- Memory allocation should be analyzed and optimized. 
- All key data transfer paths should be instrumented to collect metrics and measure performance. 
- APIs and communications interfaces need to be examined for vulnerabilities. 
- Controls and constraints should be built around memory and storage usage.
- Tests at scale should be run to determine performance limits inside of a variety of realistic simulated environments.
- Gather and construct a production payload dataset for realistic ground and mission testing.

### Integrate Space IPFS with other IPFS

Create an interoperability layer between the space-to-ground IPFS network and other IPFS networks. 

- Research IPFS gateways and relays and determine if either is an appropriate way to interface space IPFS networks with "normal" IPFS networks.
- Determine if an interop layer is better implemented as part of the ground station nodes or as a separate process.
- Implement interop layer and demonstrate exchanging data between space IPFS and "normal" IPFS.
- Consider creating an adapter to a Kubo node using the [Kubo RPI API](https://docs.ipfs.tech/reference/kubo/rpc/).

### Groundstation Network Support

Implement support for ground station networks. This will likely end up producing more architectural guidelines than opinionated implementations.

- Research methods for exchanging data across ground stations in a network, such as standing up an IPFS network for this purpose, integrating with a standard public/private IPFS network, by using a similar coordination method as the satellite constellation, etc.
- Decide on a method for tracking ground station peers and which data they have available (like a DHT).
- Design and implement APIs for tracking ground station peers and incorporating future pass information if it can predict next ground station.
- Implement data exchange method to support assembling data transmitted via satellite to multiple ground stations.
- Implement coordination method to allow ground stations to intelligently plan which data to transmit to satellites across multiple passes.

### SDKify the Project

Generalize, package, and document the project to make it easily accessible and usable in third party missions. 

- Solicit feedback from potential space/IPFS users
- Document all software pieces and create guides demonstrating how to setup a whole system and run various scenarios.
- Create example code/config/setups to demonstrate functionality.
- Define and create a rich API via static library for integration into systems built in other languages. 
- Harden CLI tool for API commands and expand functionality. 
- Examine the main interfaces across the project and determine which should and should not be generalized for user implementation.
- Potentially generalize the UDP interface to use the [Space Packet Protocol](https://egit.irs.uni-stuttgart.de/rust/spacepackets) or [AX.25](https://github.com/thombles/ax25-rs) formats to allow for more direct link integration.

### Optimize Data Transfer Protocol

Explore more sophisticated methods for exchanging data.

- Run down the protocols coming out of Move The Bytes and look for ideas which may be applicable
- Investigate chunking methods tuned to specific data types to increase the chance of deduplicating data or allow for meaningful partial transfers.
- Explore how to specify multiple blocks for request/transmission, such as by CID, sub-graph, bloom filter, etc.
- Intelligently plan which data to transfer based on passes (this will be more relevant for constellations and/or ground station networks)

### Multi-Radio Support

Extend the communication interface to support transmitting and receiving from multiple sources.

- Extend listening mode to allow for configuration of multiple listening ports and handle data appropriately
- Expand outbound communication code to allow for configuration of multiple target addresses and routing of specific packets/data types to specific target addresses
- Potentially extract sockets and ports into a more abstract CommunicationsInterface to better model multiple interfaces
- Potentially support one-way communications interfaces, such as beacon radios (transmit only)

### Satellite Constellation Support

Extend the point-to-point functionality of space-to-ground communications to also support peer-to-peer communications within a satellite constellation. 

- Decide on a method for tracking satellite peers and which data they have available (like a DHT). Likely need to support both fixed and dynamic peers.
- Design and implement APIs for tracking satellite peers and distance to peers (if available).
- Design and implement APIs for exchanging lists of available data.
- Design and implement mechanism for discovering data not currently in peer<>data list.
- Design and implement mechanism for exchange of data across constellation (assuming knowledge of distance to peers)
- Revise mechanism for constellation data exchange for optimization when knowledge of peer distance isn't directly available

### Human readable asset naming

Implement a system for providing human readable and/or static names to IPFS assets

- Determine if IPNS or DNSLink are suitable, or if a custom solution is needed
- Implement system for single IPFS assets
- Implement system for lists of IPFS assets (list of CIDs for blocks representing particular data category)

### Simulations and CI

Building out the ability to simulate space/ground test scenarios either for CI or testing purposes

- Dockerize space and ground nodes
- Determine whether to use [testground](https://docs.testground.ai/master/#/), [containernet](https://containernet.github.io/), or another test/simulation environment
- Basic setup which simulates one spacecraft to one ground station without passes and create test scenario script
- Determine which scenarios are appropriate for regular CI
- Develop the ability to simulate passes using containernet
- Develop the ability to simulate one satellite and a ground station network and create corresponding test scenario scripts
- Develop the ability to simulate a satellite constellation and a single ground station and create corresponding test scenario scripts
- Develop the ability to simulate a satellite constellation and ground station network and create corresponding test scenario scripts

### Research broader satellite scenarios

Research satellite operational scenarios and their impact on functionality.

- Research common satellites scenarios (earth observing, communications, navigation, weather, etc), and determine their operational behavior.
- Compare operational behavior from research against existing functionality and determine if any additional work is needed to support.
- Create simulation scripts to exercise software across various scenarios and determine if optimizations are required.
- Perform work to extend and optimize functionality as needed to support scenarios.