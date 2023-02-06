# Changelog

This project isn't currently versioned, so for now each PR will act as a "version".

## [ipfs-shipyard/space#21] - 2023-02-16

### Added

- Added a `local-storage` crate which exposes a `Storage` struct used to store & retrieve blocks from sqlite.
- Implemented `TransmitDag` API for transmitting blocks from storage.
- Modified receive functionality to use `local-storage` instead of streaming blocks to a file or storing incomplete blocks in-memory.
- Implemented `ImportFile` and `ExportDag` APIs for controllable importing/exporting files to & from storage.
- Added a `listen` flag to the `app-api-cli` for receiving responses to transmitted API messages.

### Fixed

- Fixed a compile issue with the `app-api-cli`