# Overview

This is documenting the pieces and steps necessary for the basic CAR transmission proof of concept. This specific step doesn't involve implementing any additional code, rather it is tying existing tools together in a very manual workflow to demonstrate generating & transmitting a CAR file from the raspberry pi to a desktop computer for receiving & unpacking.

# Setup

## Configure Raspberry PI

The raspberry pi must be configured with an ethernet connection and static IP in order to communicate with the desktop computer.

1. Connect raspberry pi via ethernet to an ethernet switch or hub.
2. Open `/etc/dhcpcd.conf` on the raspberry pi, comment out any existing network configuration, and add the following lines:
```
interface eth0
static ip_address=10.11.44.123
static routers = 10.11.44.1
```
3. Either restart the pi or run `/etc/init.d/dhcpcd restart` to reconfigure the network.

## Configure desktop

The desktop computer must also be configure with an ethernet connection and static IP to communicate with the pi.

1. Connect the desktop computer via ethernet to the same switch or hub the pi is plugged into.
1. Configure the ethernet interface to have static IP `10.11.44.124`.
1. Create a directory `poc-car-transmission` to store the local binaries & files needed

## Software setup

A few pieces of software will need to be built and configured before running this proof-of-concept exercise: [the kubos file service](https://github.com/kubos/kubos/tree/master/services/file-service), [the kubos file client](https://github.com/kubos/kubos/tree/master/clients), and [the car utility](https://github.com/ipfs-shipyard/space/tree/main/car-utility). 

### Kubos File Service

The kubos file service is a file transfer service built for satellite systems and will serve as a "simple" drop in file transfer service that can be built around in future MVPs. More information about this service can be found [here](https://docs.kubos.com/1.21.0/ecosystem/services/file.html).

This service will be run on the raspberry pi, so it will need to be cross-compiled and transferred over.

1. Clone https://github.com/kubos/kubos and navigate to `kubos/services/file-service`.
1. Build with `cross build --target armv7-unknown-linux-gnueabihf`.
1. Transfer the binary `kubos/target/armv7-unknown-linux-gnueabihf/debug/file-service` to `/home/pi/file-service` on the raspberry pi.
1. On the raspberry pi, create a file name `config.toml` in `/home/pi/` with the following contents:
```
[file-transfer-service]
downlink_ip = "10.11.44.124"
downlink_port = 8080

[file-transfer-service.addr]
ip = "10.11.44.123"
port = 8040
```

### Kubos File Client

The kubos file client will be used by the desktop computer to communicate with the file service on the raspberry pi.

1. Navigate to `kubos/clients/kubos-file-client`.
1. Run `cargo build` to build.
1. Copy the binary `kubos/target/debug/kubos-file-client` to the `poc-car-transmission` directory.


### CAR Utility

The car utility will need to be built twice: once for the raspberry pi, and once for the desktop computer.

1. Clone https://github.com/ipfs-shipyard/space and navigate to `space/car-utility`.
1. Build for raspberry pi with `cross build --target armv7-unknown-linux-gnueabihf`
1. Transfer the binary `target/armv7-unknown-linux-gnueabihf/debug/car-utility` to `/home/pi/car-utility` on the raspberry pi.
1. Build for the desktop using `cargo build`.
1. Copy the binary `target/debug/car-utility` to the `poc-car-transmission` directory.
1. Transfer the file `Cargo.toml` to `/home/pi/Cargo.toml` on the raspberry pi.

# Running the proof of concept

After running through the setup steps, the following files should exist in these directories:

`/home/pi` on the raspberry pi:
- `car-utility`
- `file-service`
- `config.toml`
- `Cargo.toml`

`poc-car-transmission` on the desktop computer:
- `car-utility`
- `kubos-file-client`

Once these files are in place, the following steps can be followed to demonstrate (very manual) end-to-end CAR handling & transmission:

On the raspberry pi:
1. Navigate to `/home/pi`.
1. Execute `./car-utility pack Cargo.toml Cargo.car`.
1. Run `./file-service -c config.toml --stdout`.

On the desktop computer:
1. Navigate to the `poc-car-utility` directory.
2. Run `./kubos-file-client -h 10.11.44.124 -P 8080 -r 10.11.44.123 -p 8040 download config.car`, it should output something like the following:
```
16:58:55 [INFO] Starting file transfer client
16:58:55 [INFO] Downloading remote: Cargo.car to local: Cargo.car
16:58:55 [INFO] -> { import, Cargo.car }
16:58:55 [INFO] <- { 116885, true, 59ef596b0585681ca63adf49da13edd2, 1, 33188 }
16:58:55 [INFO] -> { 116885, 59ef596b0585681ca63adf49da13edd2, false, [0, 1] }
16:58:55 [INFO] <- { 116885, 59ef596b0585681ca63adf49da13edd2, 0, chunk_data }
16:58:57 [INFO] -> { 116885, 59ef596b0585681ca63adf49da13edd2, true, None }
16:58:57 [INFO] -> { 116885, true, 59ef596b0585681ca63adf49da13edd2 }
16:58:57 [INFO] Operation successful
```
3. Run `./car-utility unpack Cargo.car Cargo.toml`
4. Verify the contents of `Cargo.toml` matches the contents of `space/car-utility/Cargo.toml`
