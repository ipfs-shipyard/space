# Changelog

## [0.5] - Unreleased

### Added

- Added `validate` for `StoredBlock` to leverage the validation functionality built into `beetle::iroh_unixfs::Block`.
- Added `local_storage::block::validate_dag` which determines if a list of `StoredBlock`s constitute a complete and valid DAG.
- Implemented `ValidateDag` API and added `ValidateDagResponse`. The unimplemented `ValidateBlock` was removed, as `ValidateDag` can also validate individual blocks.

### Changed

- Moved `StoredBlock` and associated implementation/tests out of the `storage` mod into a `block` mod within `local-storage`.
- Some refactoring was started in `block_streamer::server` to make testing of server functionality much easier.

### Removed

- The `RequestDag` and `RequestBlock` APIs were removed, as they were essentially equivalent to `TransmitDag` and `TransmitBlock`.

## [0.4] - 2023-02-24

### Added

- Created a general chunking struct, `SimpleChunker`, which handles chunking and assembly of any message of type `Message`. This includes a new `MessageContainer` struct which is used in the chunking of `Message`s. The `MessageContainer` essentially becomes an IPLD block containing a `Message`, which allows for verification using the `Cid` on assembly.
- The `GetMissingDagBlocks` API is now implemented based on what is currently in storage.
- Added a `local-storage` crate which exposes a `Storage` struct used to store & retrieve blocks from sqlite.
- Implemented `TransmitDag` API for transmitting blocks from storage.
- Modified receive functionality to use `local-storage` instead of streaming blocks to a file or storing incomplete blocks in-memory.
- Implemented `ImportFile` and `ExportDag` APIs for controllable importing/exporting files to & from storage.
- Added a `listen` flag to the `app-api-cli` for receiving responses to transmitted API messages.

### Changed

- Started tagging and versioning on roadmap milestones.
- Transmit and receive functionality has been modified to use the `SimpleChunker` for the transfer of Blocks. All other API communication has been modified to use the `SimpleChunker` for API messages.
- The `control` functionality in the `block-streamer` server has been renamed to `server` to better reflect it's general purpose "server-like" functionality. The code was also split up a bit to improve readability and error handling.

### Removed

- Any transmit or receive functionality dealing directly with chunks of blocks has been removed, along with associated tests.
- The `block-ship` functionality & crate was no longer needed after general message chunking was introduced.

### Fixed

- The sqlite storage provider now correctly handles importing duplicate blocks without crashing.
- The `get_missing_cid_blocks` functionality now correctly returns an error if the requested `cid` has no associated blocks (in that case we assume we haven't encountered that block yet).
- Fixed several cases where unwraps would crash the `block-streamer` on errors.
- Fixed a compile issue with the `app-api-cli`