#!/bin/sh

curl -XPOST "http://127.0.0.1:5001/api/v0/refs/local" | awk -F '"' '{print $4}' \
| xargs -I % curl -X POST "http://127.0.0.1:5001/api/v0/block/rm?arg=%"