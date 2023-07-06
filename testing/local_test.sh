#!/bin/bash -e

if ! ( uname | grep Linux )
then
  echo "This script only works on linux."
  exit 4
fi

kill_all() {
  for p in myceli controller hyphae
  do
    killall ${p} || echo "${p} is stopped"
  done
}
kill_all
if [ "${1}" = 'die' ]
then
  sleep 9999
  echo -e '\n\n\n\t###\t###\tTop-level timeout!\t###\t###\n\n'
  kill_all
  fuser "${0}" | xargs kill
  exit
fi
( "${0}" die 2>/dev/null >/dev/null <&- & ) &
cd `dirname "${0}"`/..
o=`mktemp -d`

check_log() {
  l=${2-.}
  for i in {0..9}
  do
    if [ -f ${l}/log ]
    then
      grep --color=always "${1}" ${l}/log && return
    else
      sleep 9
    fi
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
  for i in {0..9}
  do
    for p in `fuser ${c}/myceli`
    do
      echo "Pid ${p} using ${c}/myceli - kill"
      kill "${p}"
      sleep $i
    done
  done
}
start_myceli() {
  kill_myceli "${1}"
  export c="$1"
  ( ${c}/myceli "${c}/config.toml" 2>&1 <&- | tee "${c}/log" & ) &
  if [ ${c} == sat ]
  then
    check_log 'pid=' ${c}
  else
    check_log 'Listening on 0.0.0.0:' ${c}
  fi
}
port_open() {
  if echo > /dev/tcp/127.0.0.1/${1}
  then
    return 0
  else
    echo "port ${1} not yet open"
  fi
}
if port_open 5001
then
  echo "Port 5001 is already open, assuming ipfs daemon is running."
else
  echo "Starting IPFS"
  ( ipfs daemon <&- 2>/dev/null >/dev/null & ) &
fi
rm -rv sat || true
rm -rv gnd || true
mkdir -p sat/storage.db
mkdir gnd
cat > sat/config.toml <<SATCFG
listen_address = "0.0.0.0:8764"
storage_path = "sat"
SATCFG
cat > gnd/config.toml <<GNDCFG
listen_address = "0.0.0.0:8765"
storage_path = "gnd"
mtu = 1024
GNDCFG
cat > gnd/h.toml <<HYPHCFG
myceli_address= "127.0.0.1:8765"
kubo_address  = "127.0.0.1:5001"
HYPHCFG
bld() {
  to="$1"
  shift
  if [ "${1}" = '--profile' ]
  then
    profile="${2}"
  else
    profile=debug
  fi
  set -x
  cargo build --bin myceli "${@}"
  bin=`cargo metadata --format-version 1 | jq -r .target_directory`/${profile}/myceli
  cp -v "${bin}" "${to}"
}
bld gnd --features big
bld sat --profile small --features small --no-default-features
start_myceli sat
start_myceli gnd
for p in 5001 8765
do
  if ! port_open ${p}
  then
    sleep 9
  fi
done
for p in 5001 876{5,4}
do
  sleep 1
  port_open ${p}
done

controller() {
  port=${1}
  shift
  set -x
  timeout 9 cargo run --bin controller -- -l 127.0.0.1:${port} "${@}" | tee log
  set +x
}
cid_present() {
  if [ -d ${1}/storage.db ]
  then
    test -f ${1}/storage.db/cids/${2}
  else
    sqlite3 ${1}/storage.db "select * from blocks where cid = '${2}';" | grep '[a-z]'
  fi
}
transmit() {
  cid_present ${3} ${cid}
  ! cid_present ${4} ${cid}
  timeout 9 cargo run --bin controller -- 127.0.0.1:${1} transmit-dag "${cid}" 127.0.0.1:${2} 9 | tee log
  for i in {0..9}
  do
    grep -n "${cid}" log */log || true
    if cid_present ${4} ${cid}
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
  transmit 8765 8764 gnd sat
}
s2g() {
  echo "Transmit ${cid} from satellite to ground..."
  transmit 8764 8765 sat gnd
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
kill_myceli gnd

echo ', delete the storage database'
rm -rv gnd/storage.db

echo ', and start the myceli ground instance again.'
start_myceli gnd

echo 'controller software, send the TransmitDag command to the myceli space'
s2g
sleep 1

echo 'controller software, send the ValidateDag command to the myceli ground'
controller 8765 validate-dag "${cid}"
check_log 'ValidateDagResponse.*Dag.is.valid'

echo 'controller software, send the ExportDag command to the myceli ground'
controller 8765 export-dag "${cid}" "${o}/exported2"

diff "${o}/"{im,ex}ported2

echo -e '\n\n\t###\tStarting hyphae...\t###\n'
( ( cargo run --bin hyphae -- gnd/h.toml <&- 2>&1 | tee gnd/h.log & ) & ) &
echo -e '\nNow waiting for sync to Kubo...\n'
set -x
for i in {0..99}
do
  if timeout $[ 9 + i ] ipfs block get ${cid}
  then
    break
  else
    echo "${cid} not yet in Kubo"
  fi
done
ipfs block get ${cid}
ipfs dag get ${cid} | jq .

echo -e '\n\t###\tDONE\t###\n'

kill_all
