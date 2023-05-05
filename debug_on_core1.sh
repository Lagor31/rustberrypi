#!/bin/sh
#make clean all
openocd -f openocd/olimex.cfg -f openocd/rpi4.cfg 2>/dev/null  &
sleep 2 &
gdb-multiarch  --command=gdb_load_core1.cfg 
echo "Cleaning up..."
