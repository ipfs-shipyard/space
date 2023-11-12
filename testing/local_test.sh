#!/bin/bash -e

if ! ( uname | grep -q Linux )
then
  echo "This script only works on linux."
  exit 6
fi

stop() {
  #tty >/dev/null && echo stop "${@}"
  find ${2-*}/ -name "*${1}*" -type f -exec fuser '{}' \; 2>/dev/null | while read p
  do
    kill ${p}
  done
  if [ $# -eq 1 ]
  then
    killall ${1} 2>/dev/null || true
  fi
}
kill_all() {
  set +x
  for p in myceli controller hyphae watcher
  do
    stop ${p}
    killall ${p} 2>/dev/null || true # echo "${p} is stopped"
  done
  for f in {gnd,sat,ctl}/*
  do
    fuser "${f}" 2>/dev/null | xargs kill 2>/dev/null || true
  done
}
if [ "${o}" = '' ]
then
  export o=`mktemp -d`
fi
kill_all
if [ "${1}" = 'die' ]
then
  echo -n "$$" > "${o}/tl.killer.pid"
  sleep 1000
  if [ -f "${o}/tl.killer.pid" ]
  then
    if [ -f "${o}/tl.tokill.pid" ] && [ -d `cat "${o}/tl.tokill.pid"` ]
    then
      echo -e '\n\n\n\t###\t###\tTop-level timeout!\t###\t###\n\n'
    fi
    kill_all
    for f in ${o}/*.pid
    do
      kill `cat ${f}` 2>/dev/null
    done
    fuser "${0}" 2>/dev/null | xargs kill 2>/dev/null
  else
    echo "Left-over timeout abandoned."
  fi
  exit
fi
find "${TMPDIR-/tmp}/" -type f -name "tl.killer.pid" -exec cat '{}' \; 2>/dev/null | xargs kill || true
find "${TMPDIR-/tmp}/" -type f -name "tl.killer.pid" -exec rm '{}' \; 2>/dev/null || true
echo -n "$$" > "${o}/tl.tokill.pid"
( "${0}" die <&- & ) &
#cd `dirname "${0}"`/..

check_log() {
  if [ $# -lt 2 ]
  then
    echo 'Specify log directory.'
    exit 2
  fi
  l=${2}
  for i in {0..10}
  do
    if ls ${l}/${3-*}.log >/dev/null
    then
      grep --max-count=1 --color=always "${1}" ${l}/${3-*}.log && return
    else
      sleep 9
    fi
    sleep $i
  done
  echo 'Failed to find ' "${1}" ' in these logs:'
  ls -lrth --color=always ${l}/${3-*}.log
  echo ' ...elsewhere... '
  grep "${1}" */*.log
  kill_all
  exit 1
}

kill_pid() {
  for i in {0..9}
  do
    if [ -d /proc/${1}/ ]
    then
      kill ${1} || true
      sleep ${i}
    else
      return 0
    fi
  done
  echo "Failed to kill ${1}"
  exit 7
}
kill_myceli() {
  export c="$1"
  if grep -q pid= ${c}/myceli.log 2>/dev/null
  then
    kill_pid `grep pid= ${c}/myceli.log | cut -d = -f 2`
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
  stop myceli ${c}
}
start() {
  #echo start "${@}"
  [ $# -lt 2 ] && exit 9
  stop ${1} ${2}
  sleep 1
  ( 
    (
      cd "${2}"
      b=${1}
      shift
      shift
      ./${b} ${@}  > ${b}.log 2>&1 <&- &
    )  >/dev/null 2>&1 &
  ) >/dev/null 2>&1 &
  sleep 1
}
start_myceli() {
  kill_myceli "${1}"
  export c="$1"
  export RUST_LOG=trace
  sleep 1
  start myceli ${c} config.toml
  until [ -f ${c}/myceli.log ]
  do
    sleep 1
  done
  check_log 'pid=' ${c} >/dev/null
}
port_open() {
  if nc -u -z -w 9 127.0.0.1 ${1} 2>/dev/null
  then
    return 0
  elif nc  -z -w 9 127.0.0.1 ${1} 2>/dev/null
  then
   #echo "Port ${1} is TCP"
    return 0
  else
    echo "port ${1} not yet open"
    false
  fi
}
( ipfs daemon <&- >${o}/ipfs.log 2>&1 & ) >/dev/null 2>&1 &
rm -r sat || true
rm -r gnd || true
mkdir -p sat gnd ctl
configure() {
  cat > sat/config.toml <<SATCFG
  listen_address = "127.0.0.1:8764"
  radio_address  = "127.0.0.1:8765"
  storage_path = "."
  watched_directory = "watched"
  chatter_ms = ${1}
SATCFG
  cat > gnd/config.toml <<GNDCFG
  radio_address  = "0.0.0.0:8764"
  listen_address = "0.0.0.0:8765"
  storage_path = "."
  watched_directory = "watched"
  chatter_ms = ${1}
GNDCFG
  cat > gnd/hyphae.toml <<HYPHCFG
  myceli_address= "127.0.0.1:8765"
  kubo_address  = "127.0.0.1:5001"
HYPHCFG
}
configure 9876543
bld() {
  cargo build --bin ${2} --features ${3} --no-default-features --profile "${4}"
  bin=`cargo metadata --format-version 1 | jq -r .target_directory`/${4}/${2}
  cp -v "${bin}" "${1}"
}
bld gnd myceli big release
bld gnd watcher big release
bld gnd hyphae big release
bld ctl controller big release
bld sat myceli small small
bld sat watcher small small
start_myceli sat
start_myceli gnd
for i in 1{9..0}
do
  sleep $i
  for p in 5001 876{5,4}
  do
    if ! port_open ${p}
    then
      sleep $i
    fi
  done
done
for p in 5001 876{5,4}
do
  port_open ${p}
done
controller() {
  #echo "controller(${@})"
  port=${1}
  shift
  sleep 1
  if ! timeout 99 ./ctl/controller --listen-mode 127.0.0.1:${port} "${@}" 2> ctl/controller.log > ctl/output.log
  then
    echo -e "\n\t ### \t Controller command failed: \t ### \t ${*} \t ###"
    grep -n . ctl/controller.log
    grep -n . ctl/output.log
    false
  fi
}
cid_present() {
  if [ -f ${1}/cids/${2} ]
  then
    true
  elif [ -f ${1}/storage.db ]
  then
    for i in {0..9}
    do
      if sqlite3 ${1}/storage.db "select * from blocks where cid = '${2}';" | grep '[a-z]'
      then
        return 0
      fi
      sleep $i
    done
    false
  else
    false
  fi
}
other_side() {
  if [ $1 = gnd ]
  then
    echo -n sat
  elif [ $1 = sat ]
  then
    echo -n gnd
  else
    echo "fail ${0} ${*}"
    echo "fail ${0} ${*}" >&2
    exit 3
  fi
}
transmit() {
  cid_present ${3} ${cid}
  b=`other_side ${3}`
  ! cid_present ${b} ${cid}
  echo "transmit: ./ctl/controller 127.0.0.1:${1} transmit-dag \"${cid}\" 127.0.0.1:${2} 9"
  timeout 9 ./ctl/controller 127.0.0.1:${1} transmit-dag "${cid}" 127.0.0.1:${2} 9 > ctl/controller.log 2>&1
  for i in {0..9}
  do
    if cid_present ${b} ${cid}
    then
      return 0
    else
      sleep ${i}
    fi
  done
  echo "${cid} never showed up on ${b}"
  exit 8
}
port_for() {
  if [ $1 = gnd ]
  then
    echo -n 8765
  elif [ $1 = sat ]
  then
    echo -n 8764
  else
    echo "wrong params: ${0} ${*}"
    exit 4
  fi
}
g2s() {
  echo "Transmit ${cid} from ground to satellite..."
  transmit 8765 8764 gnd
}
s2g() {
  echo "Transmit ${cid} from satellite to ground..."
  transmit 8764 8765 sat
}
#ls -lrth */storage.db || date

echo -e '\n# Test Case - Verify Myceli Instances Alive'

echo '1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` ground instance.'
controller 8765 request-available-blocks
echo '- This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.'
check_log 'Received.*AvailableBlocks' ctl
echo '1. Using controller software, send the `RequestAvailableBlocks` command to the `myceli` space instance.'
controller 8764 request-available-blocks
echo '- This step passes if an `AvailableBlocks` response is received. Any other response / no response is a failure.'
check_log 'Received.*AvailableBlocks' ctl

echo -e '\n# Test Case - Transmit an IPFS File (Ground to Space)'

date > "${o}/known_good_path"

echo 'Using the controller software, send the ImportFile command to the myceli ground instance with a known good path for the one-pass payload file.'
controller 8765 import-file "${o}/known_good_path"
echo 'This step passes if an FileImported response with CID is received. Any other response / no response is a failure.'
check_log FileImported ctl

echo ' ...with the CID obtained from the FileImported response... '
export cid=`grep 'Received:.*FileImported' ctl/controller.log | tail -n 1 | cut -d '"' -f 4`
echo "... cid=${cid} ...and with the network address of the ground-to-space radio link... "
echo 'send the TransmitDag command to the myceli ground instance'
g2s

echo 'controller software, send the ValidateDag command to the myceli space instance'
controller 8764 validate-dag "${cid}"
echo 'This step passes if an ValidateDagResponse response with true. Any other response / no response is a failure.'
check_log 'ValidateDagResponse.*Dag.is.valid' ctl

echo 'controller software, send the ExportDag command to the myceli space'
controller 8764 export-dag "${cid}" "${o}/exported"
sleep 1
echo 'This step passes if the controller is able to correctly write a file to the given file path.'
diff "${o}/known_good_path" "${o}/exported"

echo -e '\n# Test Case - Transmit Back & Forth, and Export File with IPFS'

echo `uptime` `uname -a`  > "${o}/imported2"
echo 'controller software, send the ImportFile command to the myceli ground instance with a known good path for the one-pass payload file.'
controller 8765 import-file "${o}/imported2"
echo 'This step passes if an FileImported response with CID is received. Any other response / no response is a failure. ...'
check_log Received.*FileImported.*cid ctl

export cid=`grep Received.*FileImported ctl/controller.log | tail -n 1 | cut -d '"' -f 4`
echo "cid=${cid}"

echo 'Using the controller software, send the TransmitDag command to the myceli ground instance with the CID obtained from the FileImported response and with the network address of the ground-to-space radio link.'
g2s
echo 'controller software, send the ValidateDag command to the myceli space'
controller 8764 validate-dag "${cid}"
check_log 'ValidateDagResponse.*Dag.is.valid' ctl

echo 'Shutdown the myceli ground instance'
kill_myceli gnd

echo ', delete the storage database'
rm gnd/storage.db

echo ', and start the myceli ground instance again.'
start_myceli gnd

echo 'controller software, send the TransmitDag command to the myceli space'
s2g
sleep 1

echo 'controller software, send the ValidateDag command to the myceli ground'
controller 8765 validate-dag "${cid}"
check_log 'ValidateDagResponse.*Dag.is.valid' ctl

echo 'controller software, send the ExportDag command to the myceli ground'
controller 8765 export-dag "${cid}" "${o}/exported2"

diff "${o}/"{im,ex}ported2

port_open 5001
echo -e '\n\n\t###\tStarting hyphae...\t###\n'
start hyphae gnd hyphae.toml
echo -e '\nNow waiting for sync to Kubo...\n'
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

kill_all

wait_for_sync() {
  d=${2}
  check_log "${3}.*${d}${1}"          ${d} watcher
#  check_log "Imported.path.*${d}${1}" ${d} myceli
  check_log "ransmit.*Sync.*Push"     ${d} myceli
  b=`other_side ${d}`
  check_log "Sync::handle(Push(PushMsg(${d}${1}" ${b} myceli
#  check_log "Sync::handle.*Block" ${b} myceli
  p=`port_for ${b}`
  touch ${o}/notfound
  for i in {0..9}1
  do
    sleep $i
    controller ${p} --output-format json list-files
    if jq ".ApplicationAPI.AvailableDags.dags[]" ctl/output.log 2>/dev/null | grep -F --color=always "${d}${1}"
    then
      break
    fi
  done
  export cid=`jq -r ".ApplicationAPI.AvailableDags.dags[] | select( .filename == \"${d}${1}\" ).cid"  ctl/output.log`
  echo "filename=${d}${1};CID=${cid}"
  if [ "${cid}" = '' ]
  then
    jq . ctl/output.log
    exit 32
  fi
  for i in {0..9}1
  do
    controller ${p} --output-format json validate-dag ${cid}
    if jq .ApplicationAPI.ValidateDagResponse.result ctl/output.log 2>/dev/null | grep -F --color=always 'Dag is valid'
    then
      cat ctl/output.log
      rm ${o}/notfound
      break
    fi
  done
  if [ -f ${o}/notfound ]
  then
    echo "DAG for ${d}${1} never finished syncing."
    kill_all
    exit 5
  fi
  e=`pwd`/${b}/synced.${d}${1}
  controller ${p} export-dag ${cid} ${e}
  for i in {0..9}
  do
    sleep $i
    if [ ! -f ${e} ]
    then
      sleep $i
      continue
    fi
    #ls -lh ${e}
    #stat --format=%Y ${e}
    #date -d '1 second ago' +%s
    if [ `stat --format=%Y ${e}` -lt `date -d '1 second ago' +%s` ]
    then
      break
    fi
  done
  diff ${b}/synced.${d}${1} ${d}/watched/${d}${1}
}

echo -e '\n\n# Test suite: watcher'

mkdir gnd/watched sat/watched/
date > gnd/watched/gnd.prexisting.txt
date -d 'next second' > sat/watched/sat.prexisting.txt
configure 9
start_myceli sat
start_myceli gnd
start watcher gnd config.toml
start watcher sat config.toml
sleep 5
echo -e '\n  ## Test: watcher discovers pre-existing file\n'
wait_for_sync .prexisting.txt gnd 'Discovered path in'
wait_for_sync .prexisting.txt sat 'Discovered path in'

echo -e '\n  ## Test: watcher picks up moved-in file\n'
for s in gnd sat
do
  echo 'begin' > ${o}/${s}.big.txt
  yes $s `date` | head -c 2048 >> ${o}/${s}.big.txt
  echo -e '\nend' >> ${o}/${s}.big.txt
  mv ${o}/${s}.big.txt ${s}/watched/
  sleep 1
done
wait_for_sync .big.txt sat 'File modified, import:'
wait_for_sync .big.txt gnd 'File modified, import:'

echo -e '\n  ## Test: watcher picks up file written in-situ\n'
for s in gnd sat
do
  yes $s `date` | head -c 2048 >> ${s}/watched/${s}.written.txt
  sleep 1
done
wait_for_sync .written.txt sat 'File modified, import:'
wait_for_sync .written.txt gnd 'File modified, import:'

echo -e '\n\n\t###\t###\t PASSED \t###\t###\n'
kill_all
echo -e '\n\t###\t###\t DONE \t###\t###\n\n'
