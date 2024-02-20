#!/bin/bash -e
source `dirname "${0}"`/setup.env

export file="${o}/fortiming.txt"
echo "File for timing: ${file}"
gen_file() {
  echo 'File start' > "${file}"
  date >> "${file}"
  yes '' | head -n $((1024 * 1024 * 3)) >> "${file}"
  date >> "${file}"
  echo 'File end' >> "${file}"
}
await_root() {
  for i in {0..99}
  do
    s=$((i + ${1}))
    rm ctl/output.log 2>/dev/null
    if ! ls -lt "${sd}/"{names,blocks,cids}/* 2>/dev/null > "${o}/store.ls.new"
    then
      sleep ${s}
    elif ! diff "${o}/store.ls."{new,old} >/dev/null
    then
      mv -v "${o}/store.ls."{new,old}
      sleep ${s}
    elif ! controller 8763 --output-format json list-files
    then
      sleep ${s}
    elif jq '.AvailableDags.dags[].filename' ctl/output.log | grep -q '"fortiming.txt"'
    then
      return 0
    fi
    sleep ${s}
  done
  false
}
await_validate() {
  for i in {0..9}
  do
    s=$(( i + ${1} ))
    if ! await_root ${s}
    then
      sleep "${s}"
    fi
    if controller 8763 --output-format json validate-dag "${cid}" >/dev/null 2>/dev/null
    then
      return 0
    else
      sleep ${s}
    fi
  done
  false
}
do_transmit() {
  echo "do_transmit(${*})"
  export sd="sat.${1}"
  sed -i 's/8764/8763/' "${sd}/config.toml"
  sed -i 's/8765/8764/' "${sd}/config.toml"
  sed -i 's/chatter_ms.*$/chatter_ms = 12345/' "${sd}/config.toml"
  unset cid
  kill_all
  gen_file
  export RUST_LOG=trace
  while killall udp_forward
  do
    sleep 1
  done
  sleep 1
  echo "Setting up forwarder with rate ${rate}"
  cargo run --bin udp_forward -- 127.0.0.1:876{4,3,5} "${rate}" > gnd/udp_forward.log &
  rm -v gnd/storage.db || true
  start_myceli gnd
  start_myceli "${sd}"
  check_log 8764.reported.that.it.supports.${1}.protocol gnd
  check_log 8764.reported.that.it.supports.${1}.protocol "${sd}"
  controller 8765 import-file "${file}"
  imports=0
  for it in {0..99}
  do
    controller 8765 --output-format json list-files || sleep "${it}"
    sleep ${it}
    if grep fortiming.txt ctl/output.log
    then
      export cid=$(jq -r ".AvailableDags.dags[] | select( .filename == \"fortiming.txt\" ).cid"  ctl/output.log)
    fi
    if grep bafy <<< "${cid}"
    then
      echo "CID='${cid}'"
    elif [ $(( ++imports )) -gt 9 ]
    then
      echo "File ${file} never imported?!"
      exit 98
    else
      echo "Retry file import"
      controller 8765 import-file "${file}"
      sleep $(( ${it} + 9 ))
      continue
    fi
    export imports=0
    controller 8765 transmit-dag "${cid}" 127.0.0.1:8764 3 || sleep "${it}"
    sleep "${it}"
    if await_validate "${it}"
    then
      echo "Vehicle validated the transmitted DAG."
      break
    else
      echo "Retry transmit. ${it} @ " `date`
    fi
  done
  sleep 1
  controller 8763 --output-format json export-dag "${cid}" 'exported.for.timing.txt' && jq '.DagExported.path' ctl/output.log |  grep 'exported.for.timing.txt'
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
#  for appdir in gnd "${sd}"
#  do
#    for direct in send recv
#    do
#      for unit in packets bytes
#      do
#        echo "stats: ${appdir} ${direct} ${unit} " `stats ${appdir} ${direct} ${unit}`
#      done
#    done
#  done
}

export rate=19
worser=0
while [ ${worser} -le 99 ]
do
  echo "rate=${rate}"
  export rate
  kill_all
  configure 765
  do_transmit ship
  echo 'Now collect stats.'
  gsp=`stats gnd send packets`
  gsb=`stats gnd send bytes`
  grp=`stats gnd recv packets`
  grb=`stats gnd recv bytes`
  ssp=`stats sat.ship send packets`
  ssb=`stats sat.ship send bytes`
  srp=`stats sat.ship recv packets`
  srb=`stats sat.ship recv bytes`
  set +x

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

  for ship in {g,s}{s,r}{b,p}
  do
    sync=${ship}_
    echo "${ship} vs ${sync} : ${!ship} ${!sync}"
  done
  env | grep '^all' | sort
  if [ ${ssb} -lt ${ssb_} ]
  then
    echo "Vehicle-sent bytes. " $(( ( ssb_ - ssb ) * 100 / ssb_ )) '% increase ' $((++worser))
  else
    echo low = $((--worser))
  fi
  if [ ${ssp} -lt ${ssp_} ]
  then
    echo "Vehicle-sent packets. " $(( ( ssp_ - ssp ) * 100 / ssp_ )) '% increase '  $((worser += 2))
  else
    echo low = $((--worser))
  fi
  if [ ${gsb} -lt ${gsb_} ]
  then
    echo "Ground-sent bytes. " $(( ( gsb_ - gsb ) * 100 / gsb_ )) '% increase ' $((worser += 3))
  else
    echo low = $((--worser))
  fi
  if [ ${gsp} -lt ${gsp_} ]
  then
    echo "Ground-sent packets. "  $(( ( gsp_ - gsp ) * 100 / gsp_ )) '% increase ' $((worser += 4))
  else
    echo low = $((--worser))
  fi
  if [ $((gsb + ssb)) -lt $((gsb_ + ssb_)) ]
  then
    echo "Total sent bytes. "  $(( ( gsb_ + ssb_ - gsb - ssb ) * 100 / (gsb_ + ssb_) )) '% increase ' $((worser += 5))
  else
    echo low = $((--worser))
  fi
  if [ $((gsp + ssp)) -lt $((gsp_ + ssp_)) ]
  then
    echo "Total sent packets. "  $(( ( gsp_ + ssp_ - gsp - ssp ) * 100 / (gsp_ + ssp_) )) '% increase ' $((worser += 6))
  else
    echo low = $((--worser))
  fi
  echo "Test finished for rate 1:${rate}"
  export rate=$(( rate + 1 ))
done
fuser testing/local_test/timeout.killer.sh | xargs kill || true
[ ${rate} -ge 99 ]
