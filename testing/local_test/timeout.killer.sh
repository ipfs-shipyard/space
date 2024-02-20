#!/bin/bash -e

cd `dirname "${0}"`
sleep $#
parent=$(realpath /proc/${PPID}/exe)
if [ $# -lt 9 ] && [ "${1}" != sed ] && [ ${PPID} != 1 ] && ! grep -q systemd <<< "${parent}"
then
  echo daemonize killer "${@}"
  ( ./timeout.killer.sh ${PPID} "${@}" <&- 2>&1 & ) &
  exit
elif [ "${1}" != sed ]
then
  ( ./timeout.killer.sh sed "${@}" <&- 2>&1 ) 2>&1 | sed 's,^,KILLER: ,'
  exit
fi
mod=`stat --format=%Y timeout.killer.sh`

conflict() {
    if ! [ -f timeout.killer.pid ]
    then
      echo "$$" $(( ++t )) > timeout.killer.pid
    elif read pid ot < timeout.killer.pid
    then
      if [ "${pid}" = $$ ]
      then
        return 1
      elif [ -d "/proc/${pid}/" ] && [ ${ot} -ge ${t} ]
      then
        echo 'Older timeout still running'
        echo $$ 0 > timeout.killer.pid
        exit 0
      else
        rm -v timeout.killer.pid
        rm -v "${o}"/running.* || true
      fi
    else
      rm -v timeout.killer.pid
    fi
    return 0
}

while sleep $(( t += 9 ))
do
  if [ ${mod} -lt `stat --format=%Y timeout.killer.sh` ]
  then
    sleep $(( ++t ))
    ls -lth timeout.killer.sh
    echo -n "${mod}" vs ' '
    stat --format=%Y timeout.killer.sh
    echo 'timeout.killer.sh modified, recurse'
    sleep $(( ++t ))
    ./timeout.killer.sh
    exit
  fi
  if conflict
  then
    sleep $(( ++t ))
    continue
  fi
  if ! [ -f "${o}"/running.scripts.now ]
  then
    fuser *.case.sh ../../???/{myceli,controller,hyphae,watcher} > "${o}"/running.scripts.now 2>/dev/null
    rm -v "${o}"/running.tree.* 2>/dev/null || true
  elif ! diff "${o}"/running.scripts.{now,old} 2>/dev/null
  then
    mv -v "${o}"/running.scripts.{now,old}
  elif [ "${o}"/running.scripts.old -nt timeout.killer.pid ]
  then
    rm -v timeout.killer.pid
  elif ! [ -f "${o}"/running.tree.new ]
  then
    for pid in `cat "${o}"/running.scripts.old`
    do
      pstree --arguments "${pid}" | tr -d '[:digit:]' || true
      sleep $(( ++t ))
    done > "${o}"/running.tree.new
  elif ! diff "${o}"/running.tree.{new,old}
  then
    mv -v "${o}"/running.tree.{new,old}
  elif [ "${o}"/running.tree.old -nt timeout.killer.pid ]
  then
    rm -v timeout.killer.pid "${o}"/running.*.new
  elif read apid others < "${o}"/running.scripts.old
  then
    echo -e "\n \t # \t WARNING \t # "
    echo -e "\n \t # \t TIMING OUT PID ${apid} \t #"
    ps -f | grep "${apid}"
    kill "${apid}"
    rm -v "${o}"/running.scripts.old
  else
    break
  fi
done
