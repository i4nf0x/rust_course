#!/bin/bash

dir=$(dirname $0)
server="cargo run --bin server --"
client="cargo run --bin client --"

if ! [ -f "$dir/server.db" ]; then
    $server register -u Alice -p aaa || exit 1
    $server register -u Bob -p bbb || exit 1
fi

xterm -T server -e bash  -c 'cargo run --bin server -- run; read' & > /dev/null 
sleep 1
xterm -T 'Client Alice' -e bash  -c 'cargo run --bin client -- -u Alice -p aaa; read' & > /dev/null
xterm -T 'Client Bob' -e bash  -c 'cargo run --bin client -- -u Bob -p bbb; read' & > /dev/null
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
sleep 99999