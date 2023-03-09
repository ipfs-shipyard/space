# Building

This doc contains instructions on how to build binaries for various systems

## Myceli

### Docker

#### Building

The file `myceli.Dockerfile` contains all the instructions needed by Docker to produce an image for running `myceli`. This image can be built by running the following command:

    $ docker build -f myceli.Dockerfile . -t myceli

#### Running

We only suggest running `myceli` in Docker in Linux environments due to networking requirements.


Example running `myceli` in a standalone Docker container with default settings:

    $ docker run --rm -v `pwd`:`pwd` --network host -it myceli

Important pieces to point out here:

    - `-v pwd:pwd`: Mounting a local directory is necessary for `myceli`'s storage to persist
    - `--network host`: The container running `myceli` needs to either run on the host network, or on the same network as the other services which will be communicating with it (controller CLI, ground radio bridge)

Optionally you may want to pass a config file argument in with `-e CONFIG_PATH=/path/to/config.toml`