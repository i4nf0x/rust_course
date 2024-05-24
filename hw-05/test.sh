#!/bin/bash
xterm -T server -e bash  -c 'cargo run --bin server; read' & > /dev/null 
sleep 1
xterm -T client1 -e bash  -c 'cargo run --bin client; read' & > /dev/null
xterm -T client2 -e bash  -c 'cargo run --bin client; read' & > /dev/null
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
sleep 99999