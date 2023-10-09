#!/bin/bash -e
source `dirname "${0}"`/funcs.env
configure 9876543

start_myceli sat.all
start_myceli gnd

echo -e '\n\n# Test Case 0: Print Version Info\n'
controller `port_for gnd` --output-format=json request-version
jq . ctl/output.log
controller `port_for sat.all` --output-format=json request-version
jq . ctl/output.log

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
export cid=`grep 'Received.response:.*FileImported' ctl/controller.log | tail -n 1 | cut -d '"' -f 4`
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