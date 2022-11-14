# Space

## Overview

This project is focusing on applying content-addressable data & tooling to satellite communications. This is one part engineering project and one part science experiment. The overall goal is to provide a well engineered path for using pieces of IPFS, starting with content-addressable data, in the space communications context. After this has been built, a data-driven comparison must be made against existing data communications/transfer protocols. Quantifiable data must be collected for a fair comparison with existing data transfer protocols. The experimental setup will provide a structure for building up the engineered product, which will then be run through “experiments”, or test runs, to collect performance metrics.

The work and communications for this project will be coordinated through the [ipfs-shipyard/space repo](https://github.com/ipfs-shipyard/space) and the [Filecoin #space channel](https://filecoinproject.slack.com/archives/C02N7M67FKK).

## Objectives

1. Demonstrate the utility of content addressable protocols in the context of space-to-ground and space-to-space communications.
    - Provide a functional SDK for using content addressable protocols in aerospace context.
1. Provide the data required to accurately compare content addressable protocols to existing data transfer protocols.
    - Provide the tooling necessary to generate said data in real-life and simulated scenarios.

## Hypothesis

Content addressable protocols, such as those embedded in IPFS/IPLD, can provide enhanced reliability and verifiability in space-to-ground communications.

## Experiments

There are two experimental setups and scenarios which will be used in this project.

The first experimental setup addresses space-to-ground communications and consists of one satellite and N ground stations. All ground stations are assumed to be in contact with each other, whether by a private network, or access to the public internet. This assumption is drawn from the usage of IPFS between ground stations, which requires some kind of inter-station connection. The satellite will have communications with each ground station separately for X minutes each hour, or each pass. This requirement is in place to approximate expected communication patterns common to satellites. Each experiment run will see the satellite transmitting a payload of known size to the ground stations across the passes until the payload can be reassembled and verified. 

The second experimental setup addresses space-to-space communications and consists of N satellites and one ground station. All satellites are assumed to be in contact with each other via radio. Only one satellite is assumed to be in contactd with the ground station at any given time. These requirements are in place to simulate the situation where one or more satellites are used to pass data along from the ground to a satellite which cannot contact a ground station. Each experiment run will consist of the ground station transmitting a payload of known size to the one satellite it can contact, and then that satellite will trasmit the data to a different satellite for reassembly and verification.

The development/lab hardware setup for these experiments will consist of one or more Raspberry Pi 4s as the “satellites” and a desktop/laptop as the “ground station”, with a packet radio interface acting as the communications medium. The purpose of the development hardware is to provide an approximation of the target hardware platforms so that this project can develop helpful abstractions, particularly around the radio/communications interface.

Here is a list of the exact hardware pieces used for local development:
- "Satellite"
    - [Raspberry Pi 4](https://www.raspberrypi.com/products/raspberry-pi-4-model-b/)
    - [RFM69-based packet radio for raspberry pi](https://www.adafruit.com/product/4072)

- "Ground Station"
    - Laptop or Desktop running Linux or MacOS
    - [RFM69-based packet radio with USB interface](https://www.adafruit.com/product/3076)

### Tunable parameters under study:
- Number of ground stations
- Length of passes
- Passes overlapping (or not)
- Size of payload
- Size of blocks payload is broken down into
- Format of content addressable data (CAR, raw, etc)
- Speed of transmission in-pass

### Metrics to measure:
- Verification/reassembly pass or fail
- Total time from beginning of transmission to verified reassembly on ground
- Total number of passes required
- Min/max/mean number of total transmissions per block
- Min/max/mean number of block transmissions duplicated across ground stations

## Environmental Assumptions

These are assumptions we are establishing to 1) limit scope and 2) establish reasonable constraints:

- Satellites already have an established communications path to their ground stations. 
- Security or encryption is the responsibility of the satellite and already baked into their communications protocols, which we will operate under.
- Memory will be a more scarce resource than storage, and likely order(s) of magnitude less than in typical desktop/cloud computing systems.


### MVPs

- [ ] [v0.1 - Generate & transmit CAR over radio](https://github.com/ipfs-shipyard/space/issues/5)
- [ ] v0.2 - Generate DAG & transmit via stream
- [ ] v0.3 - CID request/response via DAG-stream
- [ ] v0.4 - CID request/response via tbd-protocol
- [ ] v0.5 - Basic application API and CID advertisements