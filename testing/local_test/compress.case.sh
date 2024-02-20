#!/bin/bash -e
source `dirname "${0}"`/setup.env

for m in sat.*/myceli
do
  xz -9 --keep --extreme "${m}"
  gzip  --keep --best    "${m}"
done
max_size=1000000 # 1MB (not MiB) in B

for format in {g,x}z
do
  for variant in sat.{all,sync,ship}/myceli.
  do
    fil="${variant}${format}"
    ls -lrth "${fil}"
    if [ `stat --format=%s "${fil}"` -gt ${max_size} ]
    then
      echo -e "\n\t###\t PROBLEM: \t###\t ${fil} is over ${max_size} B \t###\n"
      exit 99
    else
      export max_size=$((max_size - 40000))
    fi
  done
done
