#!/bin/sh
sudo killall openocd &
make all DEBUG_PRINTS=y
sudo openocd -f openocd/olimex.cfg -f openocd/rpi4.cfg 2>/dev/null  &
sleep 2 &
gdb-multiarch  --command=gdb_load.cfg 
echo "Cleaning up..."
