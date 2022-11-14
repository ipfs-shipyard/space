# Overview

This document sketches out a roadmap of proof of concept and minimum viable product milestones on the road of content addressable data in space 🚀.

# Roadmap 

### PoC - Iroh-based CAR utility

Create a command line utility which can pack any file into a CAR file and unpack/reconstruct the contents of a CAR file.

### PoC - Generate & transmit CAR

On the raspberry pi use the  CAR utility to generate a CAR file from a known payload. Transmit using an existing file transfer protocol over ethernet connection to laptop and reassemble original payload using CAR utility.

### PoC - Bring up radio communications

Create radio drivers on raspberry pi and desktop ends which provide a programmable interface into the radio. Create a communications service on both radio ends which provides a generic way to send and receive data over the radio. Demonstrate sending ping back and forth over radio between raspberry pi and desktop.

### MVP v0.1 - Generate & transmit CAR over radio

Generate CAR file using utility on raspberry pi. Use known file transfer protocol to transmit over radio interface to ground station. Ground station should receive, reassemble, and verify payload. Radio connection should be persistent and reliable.