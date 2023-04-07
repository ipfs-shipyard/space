# Hyphae Setup

Hyphae is a filament, or bridge, between Myceli and Kubo. It provides a pathway for the IPFS blocks inside of Myceli to flow into Kubo, and from there potentially into the broader public IPFS network.

## Running Hyphae

After building from source, or downloading a binary, `hyphae` can be run with no additional arguments:

    $ hyphae

Starting hyphae with no config file will run with a few default settings: 
- Looking for `myceli` at `127.0.0.1:8080`
- Using an MTU of 60 when communicating with `myceli`
- Looking for `kubo` at `127.0.0.1:5001`
- Syncing data every 10 seconds

Every ten seconds, `hyphae` will query `myceli` for it's available blocks, query `kubo` for it's local refs, and transfer over any blocks which exist in `myceli` and not in `kubo`.

## Configuring Hyphae

`hypahe` has a few configuration options which ship with default values, or can be tuned to fit system requirements.

Current configuration values and defaults are:
- `myceli_address` - The network address of the `myceli` instance. Defaults to `127.0.0.1:8080`.
- `kubo_address` - The network address of the `kubo` instance. Defaults to `127.0.0.1:5001`.
- `sync_interval` - Duration in milliseconds between sync operations. Defaults to 10_000 ms.
- `mtu` - The MTU used when chunking messages to/from `myceli`

These configuration values can be set via a TOML config file which is passed as an argument when running `hyphae`.

Here is an example configuration file:

    myceli_address="127.0.0.1:9090"
    kubo_address="127.0.0.1:600"
    sync_interval=30_000
    mtu=1024

If this configuration is saved to "config.toml", then we would run `hyphae config.toml` to use the config file.