# Testing Plan

This doc lays out a basic testing plan for verifying `myceli` functionality in a lab, dev, or live setup.

This doc **will not** cover any hardware or system specifics around running `myceli`. This includes radio configuration, how/where to run `myceli`, or even how to configure `myceli`. All of these specifics are assumed to be system dependent and will change based on the hardware and it's deployment configuration. 

This doc **will** cover generic behavioral testing plans for `myceli` which can be used to validate any `myceli` installation or configuration.

## Test Case - Verify Myceli Instances Alive

Steps:
1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` ground instance.
    - This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.
1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` space instance.
    - This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.

Command Details:
- `RequestAvailableBlocks` takes no parameters. A `myceli` node which receives this command will respond with an `AvailableBlocks` message to the sending address.
    - Example: `./controller myceli-service:8001 -l request-available-blocks`

This test case passes if both steps pass.

## Test Case - Transmit an IPFS File (Ground to Space)

Steps:
1. Using the controller software, send the `ImportFile` command to the `myceli` ground instance with a known good path for the one-pass payload file.
    - This step passes if an `FileImported` response with CID is received. Any other response / no response is a failure.
1. Using the controller software, send the `TransmitDag` command to the `myceli` ground instance with the CID obtained from the `FileImported` response and with the network address of the ground-to-space radio link.
1. Using the controller software, send the `ValidateDag` command to the `myceli` space instance with the CID obtained from the `FileImported` response.
    - This step passes if an `ValidateDagResponse` response with true. Any other response / no response is a failure.
1. Using the controller software, send the `ExportDag` command to the `myceli` space instance with the CID obtained from the `FileImported` response and a writeable file path.
    - This step passes if `myceli` is able to correctly write a file to the given file path.

Command Details:
- **`ImportFile`** takes one parameter, the path of the file to import. A `myceli` node which receives this command will respond with a `FileImported` message to the sending address.
    - Example: `./controller myceli-service:8001 -l import-file /path/to/file`
- **`TransmitDag`** takes three parameters: the CID of the dag to transfer, the address to transmit the dag to, and the number of retries allowed in the transfer. A `myceli` node which receives this command will begin a dag transmission session with the specified address, which is assumed to be another `myceli` node, and will not respond back to the sending address.
    - Example: `./controller myceli-service:8001 transmit-dag cid-here radio-service:8002 5`
- **`ValidateDag`** takes one parameter: the CID of the dag to validate. A `myceli` node which receives this command will respond with a `ValidateDagResponse` to the sending address, which is assumed to be another `myceli` node.
    - Example: `./controller myceli-service:8001 -l validate-dag cid-here`
- **`ExportDag`** takes two parameters: the CID of the dag to export and a filepath to export to. A `myceli` node which receives this command will not send a response back.
    - Example: `./controller myceli-service:8001 export-dag cid-here /path/to/file`

This test case passes if the final step is successful and the resulting written file matches the onboard payload file.

## Test Case - Transmit Back & Forth, and Export File with IPFS

Steps:
1. Using the controller software, send the `ImportFile` command to the `myceli` ground instance with a known good path for the one-pass payload file.
    - This step passes if an `FileImported` response with CID is received. Any other response / no response is a failure.
1. Using the controller software, send the `TransmitDag` command to the `myceli` ground instance with the CID obtained from the `FileImported` response and with the network address of the ground-to-space radio link.
1. Using the controller software, send the `ValidateDag` command to the `myceli` space instance with the CID obtained from the `FileImported` response.
    - This step passes if an `ValidateDagResponse` response with true. Any other response / no response is a failure.
1. Shutdown the `myceli` ground instance, delete the storage database, and start the `myceli` ground instance again.
1. Using the controller software, send the `TransmitDag` command to the `myceli` space instance with the CID obtained from the `FileImported` response and with the network address of the space-to-ground radio link.
1. Using the controller software, send the `ValidateDag` command to the `myceli` ground instance with the CID obtained from the `FileImported` response.
    - This step passes if an `ValidateDagResponse` response with true. Any other response / no response is a failure.
1. Using the controller software, send the `ExportDag` command to the `myceli` ground instance with the CID obtained from the `FileImported` response and a writeable file path.
    - This step passes if `myceli` is able to correctly write a file to the given file path.

Command Details:
- **`ImportFile`** takes one parameter, the path of the file to import. A `myceli` node which receives this command will respond with a `FileImported` message to the sending address.
    - Example: `./controller myceli-service:8001 -l import-file /path/to/file`
- **`TransmitDag`** takes three parameters: the CID of the dag to transfer, the address to transmit the dag to, and the number of retries allowed in the transfer. A `myceli` node which receives this command will begin a dag transmission session with the specified address, which is assumed to be another `myceli` node, and will not respond back to the sending address.
    - Example: `./controller myceli-service:8001 transmit-dag cid-here radio-service:8002 5`
- **`ValidateDag`** takes one parameter: the CID of the dag to validate. A `myceli` node which receives this command will respond with a `ValidateDagResponse` to the sending address.
    - Example: `./controller myceli-service:8001 -l validate-dag cid-here`
- **`ExportDag`** takes two parameters: the CID of the dag to export and a filepath to export to. A `myceli` node which receives this command will not send a response back.
    - Example: `./controller myceli-service:8001 export-dag cid-here /path/to/file`

This test case passes if the final step is successful and the resulting written file matches the originally transmitted payload file.