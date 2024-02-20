#!/bin/bash -e
source `dirname "${0}"`/setup.env


wait_for_sync() {
  d=${2}
  b=`other_side ${d}`
  sleep 1
  check_log "${3}.*${d}${1}" ${d} watcher || check_log "${3}.*${d}${1}" ${d} watcher
  check_log "Imported.path.*${d}${1}" ${d} myceli
  check_log "Remote.(127.0.0.1|localhost):87...reported.*supports.sync" ${d} myceli
  check_log "Remote.(127.0.0.1|localhost):87...reported.*supports.sync" ${b} myceli
  check_log "Sending.Sync.Push" ${d} myceli
  sleep 5
  check_log "Sync.:handle.Push.PushMsg.${d}${1}" ${b} myceli
  p=`port_for ${b}`
  touch ${o}/notfound
  for i in {0..9}1
  do
    sleep $i
    controller ${p} --output-format json list-files
    if jq ".AvailableDags.dags[]" ctl/output.log 2>/dev/null | grep -F --color=always "${d}${1}"
    then
      break
    fi
  done
  export cid=`jq -r ".AvailableDags.dags[] | select( .filename == \"${d}${1}\" ).cid"  ctl/output.log`
  echo "filename=${d}${1};CID=${cid}"
  if [ "${cid}" = '' ]
  then
    jq . ctl/output.log
    exit 32
  fi
  for i in {0..9}1
  do
    controller ${p} --output-format json validate-dag ${cid}
    if jq .ValidateDagResponse.result ctl/output.log 2>/dev/null | grep -F --color=always 'Dag is valid'
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
  echo "${p} Exporting ${cid} to ${e}"
  for i in {0..99}
  do
    if controller ${p} export-dag ${cid} ${e}
    then
      break
    else
      echo "Trouble exporting... could be temporary."
      sleep $i
    fi
  done
  for i in {1..99}
  do
    sleep $i
    if [ ! -f ${e} ]
    then
      sleep $i
      echo "Waiting for ${e} to be exported."
      continue
    fi
    if fuser "${e}" || [ `stat --format=%Y ${e}` -lt `date -d '1 second ago' +%s` ]
    then
      echo "Waiting for writing to finish on ${e}"
      break
    fi
  done
  set -x
  diff ${b}/synced.${d}${1} ${d}/watched/${d}${1}
  set +x
}

for sd in sat.{all,sync} # Not ship as it won't sync on its own
do
  export sd

  echo -e "\n\n# Test suite: watcher ${sd}"

  kill_all
  rm */*.log
  for rd in {gnd,sat.{all,sync,ship}}/{watched,storage.db,blocks,cids,names}
  do
    (
      rm -r "${rd}" 2>/dev/null || true
    )
  done


  mkdir -p gnd/watched ${sd}/watched/
  date > gnd/watched/gnd.prexisting.txt
  date -d 'next second' > ${sd}/watched/${sd}.prexisting.txt
  configure 7
  start_myceli ${sd}
  start_myceli gnd
  export RUST_LOG=debug
  start watcher gnd config.toml
  start watcher ${sd} config.toml
  sleep 9
  echo -e "\n  ## Test: watcher discovers pre-existing file ${sd}\n"
  wait_for_sync .prexisting.txt gnd 'Discovered path in'
  sleep 1
  wait_for_sync .prexisting.txt ${sd} 'Discovered.path in'

  echo -e '\n  ## Test: watcher picks up moved-in file\n'
  for s in gnd ${sd}
  do
    echo 'begin' > ${o}/${s}.big.txt
    yes $s `date` | head -c 2048 >> ${o}/${s}.big.txt
    echo -e '\nend' >> ${o}/${s}.big.txt
    mv ${o}/${s}.big.txt ${s}/watched/
    sleep 1
  done
  wait_for_sync .big.txt ${sd} 'File modified, import:'
  wait_for_sync .big.txt gnd 'File modified, import:'

  echo -e '\n  ## Test: watcher picks up file written in-situ\n'
  for s in gnd ${sd}
  do
    yes $s `date` | head -c 2048 >> ${s}/watched/${s}.written.txt
    sleep 1
  done
  echo "   ### From ${sd} to ground ###"
  wait_for_sync .written.txt ${sd} 'File modified, import:'
  echo "   ### From ground to ${sd} ###"
  wait_for_sync .written.txt gnd "File modified, import:"
done
