# Overview

This folder contains instructions and tools for setting up a local development environment. Specifically one involving raspberrypi-based satellites and RFM69 radio links. This readme will go over the instructions for setting up a UDP-to-radio link between a computer and a raspberry pi.

## Computer Environment

### Prerequisites

These instructions assume the following are on hand or installed:
- [Adafruit Feather 32u4 RFM69HCW Packet Radio](https://www.adafruit.com/product/3076) connected via USB
- The latest version of Rust
- The [Arduino IDE](https://www.arduino.cc/en/software)
- netcat

### Radio Setup

This will cover the installation of the radio firmware.

First, it is highly recommended to follow the [antenna setup instructions](https://learn.adafruit.com/adafruit-feather-32u4-radio-with-rfm69hcw-module/antenna-options) for the radio.

The Arduino IDE will be used to compile and install the radio firmware. Follow the [Arduino IDE setup instructions](https://learn.adafruit.com/adafruit-feather-32u4-radio-with-rfm69hcw-module/setup) and install the [RadioHead library](https://learn.adafruit.com/adafruit-feather-32u4-radio-with-rfm69hcw-module/using-the-rfm69-radio#radiohead-library-example-2328977) to prepare the IDE for usage.

Once the Arduino IDE is setup, follow these instructions to load the radio firmware:

1. Fetch the appropriate branch of the [space](https://github.com/ipfs-shipyard/space). 
1. Open the Arduino IDE, and use it to open the `space/local-dev-environment/desktop/rfm69-driver/driver` folder. 
1. Click the `Select Board` drop down at the top of the editor window and select the `Adafruit Feather 32u4` option.
   * Write down the `/dev/...` path under `Adafruit Feather 32u4` for usage later.
1. Click the green circle with right pointing arrow to compile and upload the driver to the radio.
1. A little popup should appear saying _Done Uploading_ once this process is complete.
1. Now the Arduino IDE can be closed to free up the serial port.

### Radio Service Setup

This will cover the setup of the udp-to-radio service.

1. Navigate to the `space/local-dev-environment/desktop/radio-service` directory.
1. Build the radio service with `cargo build`.
1. Start the radio service with following parameters:

    $ cargo run -- --uplink-address 127.0.0.1:8080 --downlink-address 127.0.0.1:8081 --serial-device /dev/path/from/earlier

This command configures the radio service to listen for data to uplink on the socket address `127.0.0.1:8080`, and to downlink any radio data received to the socket address `127.0.0.1:8081`. Upon starting the service should output something like this:

    UDP Uplink on:  127.0.0.1:8080
    UPD Downlink on: 127.0.0.1:8081
    Serial radio on: /dev/tty.usbmodem14201

The desktop side of the radio service is now up and ready for communication!

## Raspberry Pi Environment

### Prerequisites 

These instructions assume the following are on-hand or installed:
- [Adafruit RFM69HCW Transceiver Radio Bonnet 915Mhz](https://www.adafruit.com/product/4072)
- An ssh connection into the raspberry pi
- An internet connection for the raspberry pi

### Radio Setup

This will cover the installation of the radio bonnet and necessary system libraries for communicating with it.

First, it is highly recommended to follow the [antenna setup instructions](https://learn.adafruit.com/adafruit-radio-bonnets/antenna-options) prior to installing the radio bonnet on the raspberry pi.

After the wire antenna is installed, the radio bonnet should be mounted on the raspberry pi's header, oriented such that the _Antenna_ text is floating over the microsd port. The raspberry pi should be powered off and unplugged prior to mounting the bonnet. The pi may be powered back on after the bonnet is firmly pressed down into the header.

Once the bonnet is installed, and the pi is powered back up, follow the [Update Your Pi and Python instructions](https://learn.adafruit.com/circuitpython-on-raspberrypi-linux/installing-circuitpython-on-raspberry-pi#update-your-pi-and-python-2993452) and the [Installing CircuitPython Libraries instructions](https://learn.adafruit.com/adafruit-radio-bonnets/rfm69-raspberry-pi-setup#installing-circuitpython-libraries-3016664) to install the necessary libraries to run the radio service.

### Radio Service

This will cover the installation of the udp-to-radio service.

1. Navigate to the `space/local-dev-environment/raspberry-pi` directory.
1. Use SCP (or another file transfer mechanism) to move `service.py` to `/home/pi/service.py` on the raspberry pi.
1. Access the raspberry pi with an SSH or serial console and navigate to the `/home/pi` directory.
1. Start the radio service with the following parameters:

    $ python3 service.py 127.0.0.1:8080 127.0.0.1:8081

This command configures the radio service to listen for data to uplink on the socket address `127.0.0.1:8080`, and to downlink any radio data received to the socket address `127.0.0.1:8081`. Upon starting the service should output something like this:

    Listening for UDP traffic on 127.0.0.1:8080
    Downlinking radio data to 127.0.0.1:8081

The raspberry pi side of the radio service is now up and ready for communication!

## Verifying Radio Link

Once the desktop and raspberry pi environments have been setup and have their radio services running, the radio link can be verified. The current radio services provide a UDP interface to the radio, which means that standard network tools can be used to test connectivity. In this case the [netcat](https://netcat.sourceforge.net/) tool will be used.

On the raspberry pi, run this command to setup a netcat instance listening for traffic from the radio:

    $ nc -ul 127.0.0.1 8081

On the computer, run this command to send a UDP packet over the radio:

    $ echo "Hello Radio" | nc -u -w 0 127.0.0.1 8080

The text "Hello Radio" should appear on the raspberry pi console!