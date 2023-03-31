# Build and running Myceli in Docker

This doc contains instructions on how to build and run `myceli` in Docker

### Building

The file `myceli.Dockerfile` contains all the instructions needed by Docker to produce an image for running `myceli`. This image can be built by running the following command:

    $ docker build -f myceli.Dockerfile . -t myceli

### Pulling

The `myceli` docker images are published to the Github Container registry and can be pulled with the following command:

    $ docker pull ghcr.io/ipfs-shipyard/myceli:latest

### Running

We only suggest running `myceli` in Docker in Linux environments due to networking requirements.

Example running of `myceli` in a standalone Docker container with default settings:

    $ docker run --rm -v `pwd`:`pwd` --network host -it ghcr.io/ipfs-shipyard/myceli:latest

Important pieces to point out here:

    - `-v pwd:pwd`: Mounting a local directory is necessary for `myceli`'s storage to persist
    - `--network host`: The container running `myceli` needs to either run on the host network, or on the same network as the other services which will be communicating with it (controller CLI, ground radio bridge).

Optionally you may want to pass a config file argument in with the `CONFIG_PATH` environment variable, like this:

    $ docker run --rm -v `pwd`:/myceli/ --network host -e CONFIG_PATH=/myceli/config.toml -it ghcr.io/ipfs-shipyard/myceli:latest