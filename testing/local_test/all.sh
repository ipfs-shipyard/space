#!/bin/bash -e
cd `dirname "${0}"`
for c in *.case.sh
do
  echo -e "\n\n\t ### \t START \t ### \t ### \t Test Suite: \t ${c%.case.sh} \t ### \t###\n"
  if "./${c}"
  then
    echo -e "\n\t ### \t PASSED \t ### \t ### \t Test Suite: \t ${c%.case.sh} \t ### \t###\n\n"
  else
    echo -e "\n\t ### \t FAILED \t ### \t ### \t Test Suite: \t ${c%.case.sh} \t ### \t###\n\n"
    exit 9
  fi
done

echo -e '\n\n\t###\t###\t PASSED \t###\t###\n'

source funcs.env
kill_all

echo -e '\n\t###\t###\t DONE \t###\t###\n\n'
