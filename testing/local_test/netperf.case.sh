#!/bin/bash -e
source `dirname "${0}"`/funcs.env

file="${o}/fortiming.txt"
echo 'File start' > "${file}"
yes `date` | head -n 9999 >> "${file}"
yes | head -n 9999 >> "${file}"
yes ' ' | head -n 9999 >> "${file}"
echo 'File end' >> "${file}"
echo "File for timing: ${file}"
do_transmit() {
  export sd="sat.${1}"
  unset cid
  kill_all
  export RUST_LOG=trace
  start_myceli gnd
  start_myceli "${sd}"
  check_log 8764.reported.that.it.supports.${1}.protocol gnd
  check_log 8765.reported.that.it.supports.${1}.protocol "${sd}"
  controller 8765 import-file "${file}"
  for i in {0..9}
  do
    sleep $(( i + 9 ))
    if ! controller 8765 --output-format json list-files
    then
      sleep $(( i + 9 ))
      continue
    fi
    export cid=$(jq -r ".AvailableDags.dags[] | select( .filename == \"fortiming.txt\" ).cid"  ctl/output.log)
    if grep bafy <<< "${cid}"
    then
      echo "CID='${cid}'"
      break
    elif [ ${i} -lt 9 ]
    then
      sleep $(( i + 9 ))
    else
      echo "File never finished importing?"
      exit 83
    fi
  done
  g2s
  for i in {0..9}
  do
    sleep ${i}
    if ! controller 8764 --output-format json list-files
    then
      sleep ${i}
    elif jq -r '.AvailableDags.dags[].filename'  ctl/output.log | grep fortiming.txt
    then
      break
    else
      sleep ${i}
    fi
  done
  for i in {0..99}
  do
    sleep ${i}
    if ! controller 8764 --output-format json export-dag "${cid}" 'exported.for.timing.txt'
    then
      sleep ${i}
    elif jq '.DagExported.path' ctl/output.log |  grep 'exported.for.timing.txt'
    then
      break
    else
      sleep ${i}
    fi
  done
  for i in {0..9}
  do
    if [ -f "${sd}/exported.for.timing.txt" ]
    then
      break
    else
      echo "Waiting for ${sd}/exported.for.timing.txt"
      sleep $(( i + 9 ))
    fi
  done
  for i in {0..9}
  do
    if diff "${sd}/exported.for.timing.txt" "${file}"
    then
      break
    else
      echo "Waiting for ${sd}/exported.for.timing.txt to finish writing."
      sleep $(( i + 9 ))
    fi
  done
  if ! diff "${sd}/exported.for.timing.txt" "${file}"
  then
    echo "Transmission corrupted! ${sd}/exported.for.timing.txt != ${file}"
    exit 89
  fi
  for appdir in gnd "${sd}"
  do
    for direct in send recv
    do
      for unit in packets bytes
      do
        echo -n 'stats: '
        stats ${appdir} ${direct} ${unit}
      done
    done
  done
}

do_transmit ship

gsp=`stats gnd send packets`
gsb=`stats gnd send bytes`
grp=`stats gnd recv packets`
grb=`stats gnd recv bytes`
ssp=`stats sat.ship send packets`
ssb=`stats sat.ship send bytes`
srp=`stats sat.ship recv packets`
srb=`stats sat.ship recv bytes`

if [ ${ssp} -gt ${grp} ]
then
  echo $(( ssp - grp )) 'sat->gnd' packets lost
elif [ ${grp} -gt $(( ${ssp} + 1 )) ]
then
  echo gnd hallucinated $(( grp - ssp - 1 )) packets
  exit 9
elif [ ${gsp} -gt ${srp} ]
then
  echo $(( gsp - grp )) 'gnd->sat' packets lost
elif [ ${srp} -gt $(( ${gsp} + 1 )) ]
then
  echo sat.all hallucinated $(( srp - gsp - 1 )) packets
  exit 8
fi

do_transmit sync

gsp_=`stats gnd send packets`
gsb_=`stats gnd send bytes`
grp_=`stats gnd recv packets`
grb_=`stats gnd recv bytes`
ssp_=`stats sat.sync send packets`
ssb_=`stats sat.sync send bytes`
srp_=`stats sat.sync recv packets`
srb_=`stats sat.sync recv bytes`

export result=true
all_packets_ship=0
all_packets_sync=0
all_bytes_ship=0
all_bytes_sync=0
for ship in {g,s}{s,r}{b,p}
do
  sync=${ship}_
  echo "${ship} vs ${sync} : ${!ship} ${!sync}"
  if [ $(( 2 * ${!ship} )) -lt ${!sync} ]
  then
    echo "proto_ship wins on ${ship}"
    export result=false
  fi
  if grep -q 'p$' <<< "${ship}"
  then
    export all_packets_ship=$(( all_packets_ship + ${ship} ))
    export all_packets_sync=$(( all_packets_sync + ${sync} ))
  else
    export all_bytes_ship=$(( all_bytes_ship + ${ship} ))
    export all_bytes_sync=$(( all_bytes_sync + ${sync} ))
  fi
done
env | grep '^all' | sort
if [ ${all_packets_ship} -lt ${all_packets_sync} ]
then
  echo "SHIP wins on total packets sent in either direction!"
  result=false
else
  echo "OK on total packets sent."
fi
if [ ${all_bytes_ship} -lt ${all_bytes_sync} ]
then
  echo "SHIP wins on total bytes sent in either direction!"
  result=false
else
  echo "OK on total bytes sent."
fi
fuser testing/local_test/timeout.killer.sh | xargs kill || true
${result}
