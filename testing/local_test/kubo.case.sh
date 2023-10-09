#!/bin/bash -e

source `dirname "${0}"`/funcs.env

for i in {0..99}
do
  sleep ${i}
  if port_open 5001
  then
    break
  else
    ( ( ipfs daemon >"${o}/kubo.log" 2>&1 <&- & ) & ) &
    sleep $(( i + 9 ))
  fi
done

start_myceli gnd

date > "${o}/known_good_path"
echo 'Import a file.'
controller 8765 import-file "${o}/known_good_path"

echo -e '\n\n\t###\tStarting hyphae...\t###\n'
start hyphae gnd hyphae.toml
echo -e '\nNow waiting for sync to Kubo...\n'
for i in {0..99}
do
  export cid=`grep 'Received.response:.*FileImported' ctl/controller.log | tail -n 1 | cut -d '"' -f 4`
  if [ "${cid}" = '' ]
  then
    echo "CID not imported into myceli yet."
  elif timeout $(( 9 + i )) ipfs block get "${cid}"
  then
    break
  else
    echo "${cid} not yet in Kubo"
  fi
done
ipfs block get ${cid}
ipfs dag get ${cid} | jq .
