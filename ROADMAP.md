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

### MVP v0.2 Generate DAG & transmit via stream

Generate an in-memory DAG representation of a file on the raspberry pi. Transmit DAG as a stream over radio interface to the ground station. Ground station runs a server which receives the DAG stream, reassembles, and verifies the payload. The radio connection will be persistent and reliable, and the DAG stream will be tuned to work within the radio connection.

### MVP v0.3 CID request/response via DAG-stream

The raspberry pi will run a service which generates an in-memory DAG representation of a file, and can respond to a CID request for that DAG. The ground station sends a CID request over the radio to the raspberry pi, and the raspberry pi responds with the corresponding DAG stream. The ground station receives the DAG stream, reassembles, and verifies the payload.

### MVP v0.4 CID request/response via tbd-protocol

A protocol (like bitswap) is defined and implemented which allows for the request, response, and negotiation of the transfer of a DAG by CID. This protocol is implemented in the services created by MVP v0.3, and used to demonstrate the request of a CID by the ground, and the sucessful transmission by the raspberry pi.

### MVP v0.5 Basic application API and CID advertisements

An application API is defined and implemented for the raspberry pi which allows on-board applications to request that a file is stored in local IPFS. This includes the ability to mantain a list of CIDs stored on-board and to advertise that list to the ground. The ground station uses this advertised list to request a CID it does not currently have, and the raspberry pi transmits.