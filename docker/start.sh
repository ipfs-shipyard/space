#!/bin/bash

# Limit network traffic to 10kbit
# tc qdisc add dev eth0 root tbf rate 1kbit latency 500ms burst 1024
if [[ -z "${TEST_CMD}"  ]]; then
    `${TEST_CMD}`
fi

myceli $CONFIG_PATH