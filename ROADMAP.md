# Roadmap

## Overview

This document sketches out a roadmap of proof of concept and minimum viable product milestones on the road of content addressable data in space 🚀. 

### [PoC - CAR utility](https://github.com/ipfs-shipyard/space/issues/2)

Create a command line utility based on [Iroh](https://github.com/n0-computer/iroh) components which can pack any file into a CAR file and unpack/reconstruct the contents of a CAR file.

### [PoC - Generate & transmit CAR](https://github.com/ipfs-shipyard/space/issues/3)

Run the CAR utility on the raspberry pi to generate a CAR file from a known payload. Transmit using an existing file transfer protocol over ethernet connection to laptop and reassemble original payload using CAR utility.

### [PoC - Bring up radio communications](https://github.com/ipfs-shipyard/space/issues/4)

Create radio drivers on raspberry pi and desktop ends which provide a programmable interface into the radio. Create a communications service on both radio ends which provides a generic way to send and receive data over the radio. Demonstrate sending ping back and forth over radio between raspberry pi and desktop.

### [MVP v0.1 - Generate & transmit CAR over radio](https://github.com/ipfs-shipyard/space/issues/5)

Generate CAR file using utility on raspberry pi. Use known file transfer protocol to transmit over radio interface to ground station. Ground station should receive, reassemble, and verify payload. Radio connection should be persistent and reliable.

### PoC - Establish one way block-ship stream

Create a rough implementation of a transmitter and receiver of a stream of DAG blocks. This will provide the two ends necessary to develop and iterate on a block-ship protocol in the future. This implementation will be based on iroh, one way, no retransmissions or feedback, but it should have a tunable packet size. The file under transmission will be streamed into DAG blocks once per transmission and the blocks will not be persisted.

### MVP v0.2 Generate DAG, transmit & receive over Radio

The block-ship pieces from previous proof-of-concept will be deployed on the raspberry pi and computer and used to demonstrate sending a one-way stream of DAG blocks over the radio link. The transmitter will be tuned as appropriate to work under the dev environment's transmission limitations and slowed down to ensure successful transmission in one go.

### PoC - Investigate DAG encoding flexibility

The previous proof-of-concept and mvp should raise questions about IPFS performance and areas saturating the link budget (such as CID length). This poc is a spike beginning investigations into those questions and spinning out future work tasks around optimizing IPFS pieces for small link budgets.

### PoC - Define and implement block-ship re-transmissions

The initial implementation of the block-ship protocol is a very simple one-way stream of DAG blocks. Define a mechanism for detecting missing blocks, requesting transmission of specific blocks, and responding to those requests. Implement in block-ship transmitter and receiver.

### MVP v0.3 - Demonstrate block-ship retransmit over radio

Establish a block-ship session over the dev radio link at a speed which ensures minor packet loss and tune the re-transmission mechanism to overcome the packet loss.

### PoC - Define and implement block-ship storage mechanism

### PoC - Define and implement block-ship CID advertisements

### PoC - Define and implement block-ship CID request

### MVP v0.3 - Demonstrate block-ship request by CID

The raspberry pi will run a service which generates an in-memory DAG representation of a file, and can respond to a CID request for that file. The ground station sends a CID request over the radio to the raspberry pi, and the raspberry pi responds with the corresponding block-ship stream. The ground station receives the block-ship stream, reassembles, and verifies the payload. Radio transmission should be tuned for minor packet loss and protocol should reasonably overcome it.

### MVP v0.3 CID request/response via tbd-protocol

A protocol (like bitswap) is defined and implemented which allows for the request, response, and negotiation of the transfer of a DAG by CID. This protocol is implemented in the services created by MVP v0.3, and used to demonstrate the request of a CID by the ground, and the sucessful transmission by the raspberry pi.

### MVP v0.4 Basic application API and CID advertisements

An application API is defined and implemented for the raspberry pi which allows on-board applications to request that a file is stored in local IPFS. This includes the ability to mantain a list of CIDs stored on-board and to advertise that list to the ground. The ground station uses this advertised list to request a CID it does not currently have, and the raspberry pi transmits.