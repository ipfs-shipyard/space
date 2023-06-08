#!/bin/bash -e

if ! ( uname | grep Linux )
then
  echo "This script only works on linux."
  exit 4
fi

killall myceli || true
if [ "${1}" = 'die' ]
then
  sleep 99
  killall myceli || true
  killall controller
  exit
fi
( "${0}" die 2>/dev/null >/dev/null <&- & ) &
cd `dirname "${0}"`/..
cargo build
o=`mktemp -d`

check_log() {
  l=${2-.}
  for i in {0..9}
  do
    grep --color=always "${1}" ${l}/log && return
    sleep $i
  done
  echo 'Failed to find ' "${1}" ' in the log.'
  exit 1
}

kill_pid() {
  for i in {0..9}
  do
    if [ -d /proc/${1}/ ]
    then
      kill ${1}
      sleep ${i}
    else
      return 0
    fi
  done
  echo "Failed to kill ${1}"
  exit 2
}
kill_myceli() {
  export c="$1"
  if grep pid= ${c}/log
  then
    kill_pid `grep pid= ${c}/log | cut -d = -f 2`
  fi
}
start_myceli() {
  kill_myceli "${1}"
  export c="$1"
  ( cargo run --bin myceli -- "${c}/config.toml" 2>&1 <&- | tee "${c}/log" & ) &
  check_log 'Listening on 0.0.0.0:' ${c}
}
start_myceli sat
start_myceli grnd

controller() {
  port=${1}
  shift
  set -x
  timeout 9 cargo run --bin controller -- -l 127.0.0.1:${port} "${@}" | tee log
  set +x
}

transmit() {
  sqlite3 ${3}/storage.db "select * from blocks where cid = '${cid}';" | grep '[a-z]'
  ! sqlite3 ${4}/storage.db "select * from blocks where cid = '${cid}';" | grep '[a-z]'
  timeout 9 cargo run --bin controller -- 127.0.0.1:${1} transmit-dag "${cid}" 127.0.0.1:${2} 9 | tee log
  for i in {0..9}
  do
    if sqlite3 ${4}/storage.db "select * from blocks where cid = '${cid}';" | grep --color=always '[a-z]'
    then
      return 0
    else
      sleep ${i}
    fi
  done
  echo "${cid} never showed up on ${4}"
  exit 3
}
g2s() {
  echo "Transmit ${cid} from ground to satellite..."
  transmit 8765 8764 grnd sat
}
s2g() {
  echo "Transmit ${cid} from satellite to ground..."
  transmit 8764 8765 sat grnd
}

echo -e '\n# Test Case - Verify Myceli Instances Alive'

echo '1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` ground instance.'
controller 8765 request-available-blocks
echo '- This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.'
check_log 'Received.*AvailableBlocks'
echo '1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` space instance.'
controller 8764 request-available-blocks
echo '- This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.'
check_log 'Received.*AvailableBlocks'

echo -e '\n# Test Case - Transmit an IPFS File (Ground to Space)'

date > "${o}/known_good_path"

echo 'Using the controller software, send the ImportFile command to the myceli ground instance with a known good path for the one-pass payload file.'
controller 8765 import-file "${o}/known_good_path"
echo 'This step passes if an FileImported response with CID is received. Any other response / no response is a failure.'
check_log FileImported

echo ' ...with the CID obtained from the FileImported response... '
export cid=`grep FileImported log | cut -d '"' -f 4`
echo ' ...and with the network address of the ground-to-space radio link... '
echo 'send the TransmitDag command to the myceli ground instance'
g2s

echo 'controller software, send the ValidateDag command to the myceli space instance'
controller 8764 validate-dag "${cid}"
echo 'This step passes if an ValidateDagResponse response with true. Any other response / no response is a failure.'
check_log 'ValidateDagResponse.*Dag.is.valid'

echo 'controller software, send the ExportDag command to the myceli space'
controller 8764 export-dag "${cid}" "${o}/exported"
sleep 1
echo 'This step passes if the controller is able to correctly write a file to the given file path.'
diff "${o}/known_good_path" "${o}/exported"

echo -e '\n# Test Case - Transmit Back & Forth, and Export File with IPFS'

echo `uptime` `uname -a`  > "${o}/imported2"
echo 'controller software, send the ImportFile command to the myceli ground instance with a known good path for the one-pass payload file.'
controller 8765 import-file "${o}/imported2"
echo 'This step passes if an FileImported response with CID is received. Any other response / no response is a failure.'
check_log Received.*FileImported.*cid

export cid=`grep FileImported log | cut -d '"' -f 4`

echo 'Using the controller software, send the TransmitDag command to the myceli ground instance with the CID obtained from the FileImported response and with the network address of the ground-to-space radio link.'
g2s
echo 'controller software, send the ValidateDag command to the myceli space'
controller 8764 validate-dag "${cid}"
check_log 'ValidateDagResponse.*Dag.is.valid'

echo 'Shutdown the myceli ground instance'
kill_myceli grnd

echo ', delete the storage database'
rm -v grnd/storage.db

echo ', and start the myceli ground instance again.'
start_myceli grnd

echo 'controller software, send the TransmitDag command to the myceli space'
s2g
sleep 1

echo 'controller software, send the ValidateDag command to the myceli ground'
controller 8765 validate-dag "${cid}"
check_log 'ValidateDagResponse.*Dag.is.valid'

echo 'controller software, send the ExportDag command to the myceli ground'
controller 8765 export-dag "${cid}" "${o}/exported2"

diff "${o}/"{im,ex}ported2

echo -e '\n\t###\tDONE\t###\n'

killall myceli

