# Roadmap

## Overview

This document sketches out a roadmap of proof of concept and minimum viable product milestones on the road of content addressable data in space 🚀. 

## Milestone 1

Milestone one focuses on basic system and hardware bring up.

### [PoC - CAR utility](https://github.com/ipfs-shipyard/space/issues/2)

Create a command line utility based on [Iroh](https://github.com/n0-computer/iroh) components which can pack any file into a CAR file and unpack/reconstruct the contents of a CAR file.

### [PoC - Generate & transmit CAR](https://github.com/ipfs-shipyard/space/issues/3)

Run the CAR utility on the raspberry pi to generate a CAR file from a known payload. Transmit using an existing file transfer protocol over ethernet connection to laptop and reassemble original payload using CAR utility.

### [PoC - Bring up radio communications](https://github.com/ipfs-shipyard/space/issues/4)

Create radio drivers on raspberry pi and desktop ends which provide a programmable interface into the radio. Create a communications service on both radio ends which provides a generic way to send and receive data over the radio. Demonstrate sending ping back and forth over radio between raspberry pi and desktop.

### [MVP v0.1 - Generate & transmit CAR over radio](https://github.com/ipfs-shipyard/space/issues/5)

Generate CAR file using utility on raspberry pi. Use known file transfer protocol to transmit over radio interface to ground station. Ground station should receive, reassemble, and verify payload. Radio connection should be persistent and reliable.

## Milestone 2

Milestone two focuses on creating a bare minimum method of transferring a file using IPFS components.

### PoC - Establish one way block-ship stream

Create a rough implementation of a transmitter and receiver of a stream of DAG blocks. This will provide the two ends necessary to develop and iterate on a block-ship protocol in the future. This implementation will be based on iroh, one way, no retransmissions or feedback, but it should have a tunable packet size. The file under transmission will be streamed into DAG blocks once per transmission and the blocks will not be persisted.

### Poc - Implement baseline metrics

Implement a baseline set of metrics, perhaps [this list](https://github.com/n0-computer/test-plans/tree/main/movethebytes/data-transfer#metrics) suggested by move the bytes, and build into logging of block-ship transmitter and receiver.

### MVP v0.2 - Generate DAG, transmit & receive over Radio

The block-ship pieces from previous proof-of-concept will be deployed on the raspberry pi and computer and used to demonstrate sending a one-way stream of DAG blocks over the radio link. The transmitter will be tuned as appropriate to work under the dev environment's transmission limitations and slowed down to ensure successful transmission in one go. Metrics should be recorded and parsed out of logs.

## Milestone 3

Milestone three focuses on implementing minimum retry methods for reliability.

### PoC - Investigate DAG encoding flexibility

The previous proof-of-concept and mvp should raise questions about IPFS performance and areas saturating the link budget (such as CID length). This poc is a spike beginning investigations into those questions and spinning out future work tasks around optimizing IPFS pieces for small link budgets.

### PoC - Define and implement block-ship re-transmissions

The initial implementation of the block-ship protocol is a very simple one-way stream of DAG blocks. Define a mechanism for detecting missing blocks, requesting transmission of specific blocks, and responding to those requests. Implement in block-ship transmitter and receiver.

### MVP v0.3 - Demonstrate block-ship retransmit over radio

Establish a block-ship session over the dev radio link at a speed which ensures minor packet loss and tune the re-transmission mechanism to overcome the packet loss.

## Milestone 4

Milestone four focuses on creating a mechanism for requesting transfers.

### PoC - Define and implement block-ship storage mechanism

Define and implement (or borrow) a method for chunking files into DAG blocks and storing said blocks in persistent storage. Chunked files should be retrievable by CID.

### PoC - Define and implement block-ship CID request

Define and implement a mechanism for block-ship to transmit & receive a CID request, and to respond to CID request with appropriate previously stored blocks.

### MVP v0.4 - Demonstrate block-ship request by CID

The raspberry pi will run a service which generates an in-memory DAG representation of a file, and can respond to a CID request for that file (CID will be shared ahead of time). The ground station sends a CID request over the radio to the raspberry pi, and the raspberry pi responds with the corresponding block-ship stream. The ground station receives the block-ship stream, reassembles, and verifies the payload. Radio transmission should be tuned for minor packet loss and protocol should reasonably overcome it.

## Milestone 5

Milestone five focuses on creating mechanisms for advertising available data and interacting with outside applications.

### PoC - Define and implement block-ship CID advertisements

Define and implement a mechanism for block-ship to advertise which CIDs are available for request/transmission.

### Poc - Define and implement basic spacecraft application API

Define an interface and API by which other spacecraft applications/services can interact with the on-board IPFS instance to perform actions such as storing/chunking a file and requesting transmission of a file.

### MVP v0.5 - Basic application API and CID advertisements

An application API is defined and implemented for the raspberry pi which allows on-board applications to request that a file is stored in local IPFS. This includes the ability to mantain a list of CIDs stored on-board and to advertise that list to the ground. The ground station uses this advertised list to request a CID it does not currently have, and the raspberry pi transmits.

## Future Milestones

### PoC - Initial communications API

Define and document current method of integrating block-ship pieces into satellite and ground communications layers. Extract out UDP packet stuffing into abstract interface to allow for other packet formats in other use cases.

### PoC - Explore various satellite profiles

Satellites operate across a variety of scenarios (earth observing, communications, navigation, weather, etc), each of which may require a different operational profile. Research these common scenarios, and their associated operational profiles, and create simulation scenarios to test against IPFS software.

## Future ideas

- Utilizing [testground](https://docs.testground.ai/) and [containernet](https://containernet.github.io/) for larger scale simulations
- Explore using the [Space Packet Protocol](https://egit.irs.uni-stuttgart.de/rust/spacepackets) or [AX.25](https://github.com/thombles/ax25-rs) for payload format
- Keep an eye on the output of the [Move the Bytes WG](https://www.notion.so/Move-the-Bytes-WG-2dc2194a004f4b72a9706ad5a150081d) and see if we can steal/borrow their protocol ideas